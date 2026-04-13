# Full-Text Search

cozo-redb ships full-text search via [tantivy](https://github.com/quickwit-oss/tantivy),
enabled by the `fts` Cargo feature. The default `compact` feature set pulls
in `fts-mmap`, which uses an on-disk tantivy directory next to the redb file
and is the right choice for native embeddings. The plain `fts` feature uses
an in-RAM tantivy index instead — slower to warm up because every database
open rebuilds the index, but it has no filesystem dependencies and is the
mode used by the WASM build. This document covers the user-facing
CozoScript syntax; for internals see `cozo-core/src/fts/mod.rs`.

## Creating an index

```cozoscript
{:create notes {id: Int => title: String, body: String}}

::fts create notes:ft { fields: [title, body] }
```

`::fts create base:idx_name { fields: [...] }` builds a tantivy sidecar index
over the named text columns of `base`. The index name follows the same
`relation:index` convention as `::hnsw`. Any existing rows are indexed at
creation time.

Indexed columns must be `String`; non-string values in the named columns are
silently skipped rather than erroring, so a mixed relation can still be
indexed without scrubbing the schema.

### Sidecar layout

With the `fts-mmap` feature (the native default), the tantivy directory
lives next to the redb file:

```
my.db
my.db.ft/<relation>/<index>/
```

Backups: close the database, then copy both `my.db` and `my.db.ft/` together.

With the plain `fts` feature (the WASM build, or `default-features = false,
features = ["fts"]`), there is no sidecar — the tantivy index is held in
RAM and discarded when the `Db` is dropped. Reopening the database starts
with an empty FTS index that needs to be repopulated.

## Searching

```cozoscript
?[id, score] := ~notes:ft{ id | query: "+rust -snake", k: 10, bind_score: score }
```

The left side of `|` lists the base-relation columns to bind. Any column not
mentioned is ignored. The right side carries the search parameters:

| Parameter    | Required | Type            | Meaning                                    |
|--------------|----------|-----------------|--------------------------------------------|
| `query`      | yes      | string literal  | Lucene-style query (see below)             |
| `k`          | yes      | int literal     | Max hits to return                         |
| `bind_score` | no       | binding         | Receives the tantivy relevance score (f64) |

### Query syntax

`query` is passed verbatim to tantivy's
[`QueryParser`](https://docs.rs/tantivy/0.22/tantivy/query/struct.QueryParser.html),
so the full Lucene-style language is available:

- `term1 term2` — OR by default
- `+required -excluded` — require / exclude terms
- `"exact phrase"` — phrase match
- `"phrase"~2` — phrase with slop
- `title:rust` — restrict to a specific field
- `rust^2 graph` — term boost
- `gra*` — wildcard (tantivy limitations apply)

The parser searches all indexed text fields by default.

## Write semantics

Inserts, updates, and deletes on a relation that has FTS indices flow through
to tantivy as part of the same CozoScript statement. The tantivy writer
buffers the change and commits it only after the underlying redb transaction
commits successfully, so the two stores stay in step on the happy path.

A failed cozo transaction leaves tantivy's buffered writes uncommitted; the
next successful commit will flush any leftover buffered state. If strict
rollback semantics matter for your workload, open a fresh `Db` after a failed
transaction.

## Dropping an index

```cozoscript
::fts drop notes:ft
```

This removes the manifest from the relation handle and deletes the sidecar
directory. The base relation is untouched.

## Limitations

- `query` and `k` must be literals; parameters are not yet supported.
- Schema changes on an existing FTS index are not auto-detected — drop and
  recreate the index if you change which columns are indexed.
- Only `String`-typed columns are indexed; other scalar types are dropped
  silently on write.
- There is no `MinHash-LSH` replacement. If you need approximate set
  similarity, tantivy's `MoreLikeThisQuery` is a reasonable starting point.
