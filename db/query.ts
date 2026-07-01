// Re-exports drizzle-orm's query builder helpers so consumers (the Worker,
// the Mac) build queries against the SAME drizzle-orm module instance that
// schema.ts's tables were declared with. drizzle-orm brands its Column/SQL
// types with private/protected fields, so a second independently-installed
// copy of drizzle-orm produces types that fail to structurally match this
// package's `entities`/`edges`/... tables - import from here, not from
// "drizzle-orm" directly, in any package that also depends on @lifeos/db.
export { and, asc, desc, eq, isNull, lte, or, sql } from "drizzle-orm";
