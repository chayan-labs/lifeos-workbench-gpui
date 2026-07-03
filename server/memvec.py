#!/usr/bin/env python3
"""memvec for Life OS - the semantic half of hybrid recall.

Adapted from the harness `~/.claude/bin/memvec.py` (reused unchanged in spirit:
MiniLM-384 + sqlite-vec vec0), retargeted to the un-synced `lifeos-derived.db`
and given a CLI so `lifeos-api` can shell out to it. The Rust side owns the FTS5
lexical half and fuses results with RRF; this owns `entity_vec` because the vec0
loadable extension is NOT available to the Rust libSQL build.

Honest degradation: if sentence-transformers / sqlite-vec are not installed, we
print a clear message to stderr and exit non-zero so the API falls back to
lexical-only search.

Subcommands (all take --db <derived.db>):
  query   --workspace W --text "..." [--k 20]   -> prints `id<TAB>distance` per line
  embed   --workspace W --id ENT --text "..."    -> upsert one entity's vector
  rebuild --canonical lifeos.db [--workspace W]  -> re-embed all entities
"""
import argparse
import struct
import sys

DIM = 384
MODEL_NAME = "all-MiniLM-L6-v2"

_MODEL = None


def die(msg: str, code: int = 3):
    print(f"memvec: {msg}", file=sys.stderr)
    sys.exit(code)


def model():
    """Lazily load the embedder; fail loudly (non-zero) if deps are missing."""
    global _MODEL
    if _MODEL is None:
        try:
            from sentence_transformers import SentenceTransformer
        except ImportError:
            die("sentence-transformers not installed (pip install sentence-transformers)")
        _MODEL = SentenceTransformer(MODEL_NAME)
    return _MODEL


def embed_text(text: str) -> bytes:
    vec = model().encode([text], normalize_embeddings=True)[0]
    return struct.pack(f"{DIM}f", *vec)


def connect(db_path: str):
    """Open the derived DB with sqlite-vec loaded and the vec schema ensured."""
    import sqlite3

    try:
        import sqlite_vec
    except ImportError:
        die("sqlite-vec not installed (pip install sqlite-vec)")

    conn = sqlite3.connect(db_path)
    conn.enable_load_extension(True)
    sqlite_vec.load(conn)
    conn.enable_load_extension(False)
    conn.execute(
        f"CREATE VIRTUAL TABLE IF NOT EXISTS entity_vec USING vec0(embedding float[{DIM}])"
    )
    conn.execute(
        "CREATE TABLE IF NOT EXISTS entity_vec_meta "
        "(rowid INTEGER PRIMARY KEY, id TEXT UNIQUE, workspace_id TEXT)"
    )
    return conn


def upsert(conn, workspace_id: str, entity_id: str, text: str):
    row = conn.execute(
        "SELECT rowid FROM entity_vec_meta WHERE id = ?", (entity_id,)
    ).fetchone()
    blob = embed_text(text)
    if row:
        rowid = row[0]
        conn.execute("UPDATE entity_vec SET embedding = ? WHERE rowid = ?", (blob, rowid))
        conn.execute(
            "UPDATE entity_vec_meta SET workspace_id = ? WHERE rowid = ?",
            (workspace_id, rowid),
        )
    else:
        cur = conn.execute("INSERT INTO entity_vec(embedding) VALUES (?)", (blob,))
        rowid = cur.lastrowid
        conn.execute(
            "INSERT INTO entity_vec_meta(rowid, id, workspace_id) VALUES (?, ?, ?)",
            (rowid, entity_id, workspace_id),
        )
    conn.commit()


def cmd_query(args):
    conn = connect(args.db)
    blob = embed_text(args.text)
    rows = conn.execute(
        "SELECT v.rowid, v.distance FROM entity_vec v "
        "WHERE v.embedding MATCH ? AND k = ? ORDER BY v.distance",
        (blob, args.k),
    ).fetchall()
    for rowid, distance in rows:
        meta = conn.execute(
            "SELECT id, workspace_id FROM entity_vec_meta WHERE rowid = ?", (rowid,)
        ).fetchone()
        if not meta:
            continue
        ent_id, ws = meta
        if args.workspace and ws != args.workspace:
            continue
        print(f"{ent_id}\t{distance}")


def cmd_embed(args):
    conn = connect(args.db)
    upsert(conn, args.workspace, args.id, args.text)


def cmd_rebuild(args):
    import sqlite3

    conn = connect(args.db)
    conn.execute("DELETE FROM entity_vec")
    conn.execute("DELETE FROM entity_vec_meta")
    conn.commit()

    src = sqlite3.connect(args.canonical)
    sql = "SELECT id, workspace_id, coalesce(title,''), attrs FROM entities"
    params = ()
    if args.workspace:
        sql += " WHERE workspace_id = ?"
        params = (args.workspace,)
    n = 0
    for ent_id, ws, title, attrs in src.execute(sql, params).fetchall():
        text = f"{title} {attrs}".strip()
        upsert(conn, ws, ent_id, text)
        n += 1
    print(f"memvec: embedded {n} entities", file=sys.stderr)


def main():
    parser = argparse.ArgumentParser(description="Life OS memvec (semantic recall)")
    parser.add_argument("--db", required=True, help="path to lifeos-derived.db")
    sub = parser.add_subparsers(dest="cmd", required=True)

    q = sub.add_parser("query")
    q.add_argument("--workspace")
    q.add_argument("--text", required=True)
    q.add_argument("--k", type=int, default=20)
    q.set_defaults(func=cmd_query)

    e = sub.add_parser("embed")
    e.add_argument("--workspace", required=True)
    e.add_argument("--id", required=True)
    e.add_argument("--text", required=True)
    e.set_defaults(func=cmd_embed)

    r = sub.add_parser("rebuild")
    r.add_argument("--canonical", required=True)
    r.add_argument("--workspace")
    r.set_defaults(func=cmd_rebuild)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
