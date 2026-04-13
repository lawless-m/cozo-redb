# Differences from upstream CozoDB

`cozo-redb` is a narrowed fork of [cozodb/cozo](https://github.com/cozodb/cozo).
The query engine, Datalog semantics, time-travel relations, and HNSW vector
search are inherited unchanged, and much of the
[upstream documentation](https://docs.cozodb.org/) still describes how queries
work here. This page is the full reckoning of where this fork and upstream no
longer line up.

## Not a takeover bid

This fork is not a community takeover, not a claim on the CozoDB name, and
makes no promise of support, release cadence, or stability for anyone else.
It is a personal maintenance furrow with a specific angle: redb is the
interesting backend upstream never shipped, and the author wants a tight
Rust-first graph database to use. Upstream has been dormant since December
2024; pick this fork up only if the specific shape of "redb + Datalog + time
travel + vector search, nothing else" is what you need.

## Storage backends removed

Upstream shipped five persistent storage backends. This fork ships one.

In one long day the fork was stripped of:

* the `cozorocks` C++ FFI subcrate and its 42 MB vendored `librocksdb`
  submodule;
* the `sled`, `tikv`, `sqlite`, and `rocksdb` storage backends.

Remaining backends: **`mem`** (in-process, non-persistent) and **`redb`**
(pure-Rust mmap B-tree, persistent, ACID, supports time travel). That is the
lot.

## Language bindings removed

The Python, Node, Java, Swift, and C language bindings were removed. Upstream
v0.7 still has them if you need them.

This fork targets Rust embedders exclusively. The only non-Rust surface is
`cozo-lib-wasm`, a `wasm-pack` build that exposes a `CozoDb` JS class over
the in-memory storage backend, with optional in-RAM full-text search.

## No client-server mode

Upstream CozoDB once offered a client-server mode via an HTTP server. This
fork does not. If you need a client-server graph database, `cozo-redb` is not
for you.

## No backup / restore API

There is no in-process `backup_db` / `restore_backup` / `import_from_backup`
API. The old upstream backup mechanism relied on sqlite as an intermediate
format; when sqlite was dropped from this fork, the backup API went with it.

To back up a redb database: close it and copy the `.redb` file with your
usual backup tool (`cp`, `rsync`, `restic`, etc). To restore: copy the file
back. redb is a single file and "copy the file" is a perfectly good backup
strategy.

## Full-text search modified

The full-text index has been modified in this fork — it differs from the
upstream FTS, and upstream documentation covering FTS is no longer accurate
here.

## MinHash-LSH removed

Upstream offered a MinHash-LSH index for approximate set similarity. This
fork removed it entirely. Upstream pages describing MinHash-LSH do not apply
here.

## What was added

* A pure-Rust `redb` storage backend with time travel.
* Benchmark infrastructure that actually runs.
* A CI pipeline covering build, cross-platform, docs, fmt, clippy, audit,
  and dependabot.
