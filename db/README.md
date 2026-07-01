# @lifeos/db

Drizzle ORM schema shared by the Worker and the Mac. Mirrors `migrations/0001_core.sql`
and `migrations/0002_control_plane.sql` 1:1 - the SQL files are the source of truth and are
applied directly (no drizzle-kit migration generation); this package exists for typed
query access only. Keep `schema.ts` in sync by hand when those migrations change.

Does not cover `migrations/0003_derived.sql` - that schema belongs to the separate,
never-synced derived DB (FTS5/vector search state) and is owned by `lifeos-api`/`memvec.py`,
not Drizzle. See `docs/DATA-MODEL.md` §5.

## Usage

```ts
// Cloudflare Worker (HTTP-only transport)
import { createWorkerDb } from "@lifeos/db/client/worker";
const db = createWorkerDb({ url: env.TURSO_URL, authToken: env.TURSO_TOKEN });

// Mac (embedded replica, local-first)
import { createMacDb } from "@lifeos/db/client/mac";
const db = createMacDb({
  path: "file:lifeos.db",
  syncUrl: process.env.TURSO_URL,
  authToken: process.env.TURSO_TOKEN,
});
```

Both return a `drizzle()` instance bound to the shared `schema.ts` tables - query with the
Drizzle query builder, not raw SQL, at JS call sites.

**Always import query builders (`and`/`eq`/`sql`/...) from `@lifeos/db/query`, never from
`"drizzle-orm"` directly**, in any package that also depends on `@lifeos/db` (issue #64).
drizzle-orm brands its `Column`/`SQL` types with private/protected fields; a second,
independently-installed copy of `drizzle-orm` (which npm gives you by default across
separate `package.json`s with no workspace hoisting) produces types that fail to
structurally match this package's tables, even though the code is otherwise correct.

`./client/local` (`createLocalDb`) spins up a throwaway in-memory or file-backed SQLite DB
against this package's own schema/drizzle-orm instance - for tests only (see
`worker/test/entities.test.ts`), not a third production tier alongside `client.worker.ts`/
`client.mac.ts`.

## Build

```
npm install
npm run build   # emits dist/ (gitignored) - tsc compiles schema.ts/client.*.ts to JS + .d.ts
npm run check   # type-check only, no emit
```
