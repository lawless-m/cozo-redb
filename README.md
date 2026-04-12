[![CI](https://github.com/lawless-m/cozo-redb/actions/workflows/build.yml/badge.svg)](https://github.com/lawless-m/cozo-redb/actions/workflows/build.yml)
[![License](https://img.shields.io/github/license/lawless-m/cozo-redb)](https://github.com/lawless-m/cozo-redb/blob/main/LICENSE.txt)

# `cozo-redb`

> **A Rust graph database backed by [redb](https://github.com/cberner/redb).**
>
> `cozo-redb` is an aggressive fork of [cozodb/cozo](https://github.com/cozodb/cozo) —
> Ziyang Hu's transactional Datalog database, which has been dormant since December 2024.
> This fork picks **redb** (a pure-Rust mmap B-tree) as its single persistent backend and
> deletes everything else. The query language, Datalog semantics, time-travel relations,
> and HNSW vector search are inherited unchanged from upstream; the full-text index has
> been modified, and MinHash-LSH has been removed entirely. The
> [upstream documentation](https://docs.cozodb.org/) and its mirrors (readthedocs, docs.rs
> for upstream crates) still describe much of how queries work here, though they are no
> longer perfectly in step with this fork.
>
> This is not a takeover bid or a bid for the upstream name. It's a personal maintenance
> furrow with a specific angle: **redb is the interesting backend upstream never shipped,
> and we want a tight Rust-first graph database to use.**
>
> In one long day this fork was stripped of:
>
> * the `cozorocks` C++ FFI subcrate and its 42 MB vendored `librocksdb` submodule;
> * the `sled`, `tikv`, `sqlite`, and `rocksdb` storage backends;
> * the `backup_db` / `restore_backup` / `import_from_backup` API surface (since it
>   was sqlite-coupled — to back up a redb database, close it and copy the file);
> * the Python, Node, Java, Swift, and C language bindings (upstream v0.7 still has
>   them if you need them).
>
> What was added: a pure-Rust `redb` storage backend with time travel, benchmark
> infrastructure that actually runs, and a CI pipeline covering build / cross-platform /
> docs / fmt / clippy / audit / dependabot.
>
> Remaining backends: **`mem`** (in-process, non-persistent) and **`redb`** (pure-Rust
> mmap B-tree, persistent, ACID, supports time travel). That's it.
>
> No support commitment, no release cadence promise. MPL-2.0, PRs welcome.

### Table of contents

1. [Introduction](#Introduction)
2. [Getting started](#Getting-started)
3. [Install](#Install)
4. [Architecture](#Architecture)
5. [Status of the project](#Status-of-the-project)
6. [Links](#Links)
7. [Licensing and contributing](#Licensing-and-contributing)

## Introduction

CozoDB is a general-purpose, transactional, relational database
that uses **Datalog** for query, is **embeddable** but can also handle huge amounts of data and concurrency,
and focuses on **graph** data and algorithms.
It supports **time travel** and it is **performant**!

### What does _embeddable_ mean here?

A database is almost surely embedded
if you can use it on a phone which _never_ connects to any network
(this situation is not as unusual as you might think). SQLite is embedded. MySQL/Postgres/Oracle are client-server.

> A database is _embedded_ if it runs in the same process as your main program.
> This is in contradistinction to _client-server_ databases, where your program connects to
> a database server (maybe running on a separate machine) via a client library. Embedded databases
> generally require no setup and can be used in a much wider range of environments.

Upstream CozoDB also offered a client-server mode via an HTTP server; **this fork is
embedded-only** — the HTTP server has been removed along with the other non-redb
backends, and `cozo-redb` runs exclusively in the same process as your program.

### Why _graphs_?

Because data are inherently interconnected. Most insights about data can only be obtained if
you take this interconnectedness into account.

> Most existing _graph_ databases start by requiring you to shoehorn your data into the labelled-property graph model.
> We don't go this route because we think the traditional relational model is much easier to work with for
> storing data, much more versatile, and can deal with graph data just fine. Even more importantly,
> the most piercing insights about data usually come from graph structures _implicit_ several levels deep
> in your data. The relational model, being an _algebra_, can deal with it just fine. The property graph model,
> not so much, since that model is not very composable.

### What is so cool about _Datalog_?

Datalog can express all _relational_ queries. _Recursion_ in Datalog is much easier to express,
much more powerful, and usually runs faster than in SQL. Datalog is also extremely composable:
you can build your queries piece by piece.

> Recursion is especially important for graph queries. CozoDB's dialect of Datalog
> supercharges it even further by allowing recursion through a safe subset of aggregations,
> and by providing extremely efficient canned algorithms (such as PageRank) for the kinds of recursions
> frequently required in graph analysis.
>
> As you learn Datalog, you will discover that the _rules_ of Datalog are like functions
> in a programming language. Rules are composable, and decomposing a query into rules
> can make it clearer and more maintainable, with no loss in efficiency.
> This is unlike the monolithic approach taken by the SQL `select-from-where` in nested forms,
> which can sometimes read like [golfing](https://en.wikipedia.org/wiki/Code_golf).

### Time travel?

Time travel in the database setting means
tracking changes to data over time
and allowing queries to be logically executed at a point in time
to get a historical view of the data.

> In a sense, this makes your database _immutable_,
> since nothing is really deleted from the database ever.
>
> In Cozo, instead of having all data automatically support
> time travel, we let you decide if you want the capability
> for each of your relation. Every extra functionality comes
> with its cost, and you don't want to pay the price if you don't use it.
>
Cozo lets you enable time travel per relation, so you only pay the cost on the data that needs it.

### How performant?

This fork targets embedded, single-box workloads. On the retired multi-backend version,
benchmarks (`cozo-core/BENCHMARKS.md`) showed redb beating sqlite on every read and
aggregation workload by 32–49%, with time-travel aggregation over a 1M-row relation
2.35× faster, which is why redb was kept as the sole persistent backend.

## Getting started

The query language reference (tutorial, execution model, built-in functions) is still hosted
at the original upstream docs site — this fork has not replicated it. Start with the
[tutorial](https://docs.cozodb.org/en/latest/tutorial.html), then see the
[execution model](https://docs.cozodb.org/en/latest/execution.html). Most of what is
described there still applies to this fork's query engine, but mind that the full-text
index has been modified and MinHash-LSH has been removed altogether; the upstream pages
covering those are no longer accurate here.

### Teasers

If you are in a hurry and just want a taste of what querying with CozoDB is like, here it is.
In the following `*route` is a relation with two columns `fr` and `to`,
representing a route between those airports,
and `FRA` is the code for Frankfurt Airport.

How many airports are directly connected to `FRA`?

```
?[count_unique(to)] := *route{fr: 'FRA', to}
```

| count_unique(to) |
|------------------|
| 310              |

How many airports are reachable from `FRA` by one stop?

```
?[count_unique(to)] := *route{fr: 'FRA', to: stop},
                       *route{fr: stop, to}
```

| count_unique(to) |
|------------------|
| 2222             |

How many airports are reachable from `FRA` by any number of stops?

```
reachable[to] := *route{fr: 'FRA', to}
reachable[to] := reachable[stop], *route{fr: stop, to}
?[count_unique(to)] := reachable[to]
```

| count_unique(to) |
|------------------|
| 3462             |

What are the two most difficult-to-reach airports
by the minimum number of hops required,
starting from `FRA`?

```
shortest_paths[to, shortest(path)] := *route{fr: 'FRA', to},
                                      path = ['FRA', to]
shortest_paths[to, shortest(path)] := shortest_paths[stop, prev_path],
                                      *route{fr: stop, to},
                                      path = append(prev_path, to)
?[to, path, p_len] := shortest_paths[to, path], p_len = length(path)

:order -p_len
:limit 2
```

| to  | path                                                | p_len |
|-----|-----------------------------------------------------|-------|
| YPO | `["FRA","YYZ","YTS","YMO","YFA","ZKE","YAT","YPO"]` | 8     |
| BVI | `["FRA","AUH","BNE","ISA","BQL","BEU","BVI"]`       | 7     |

What is the shortest path between `FRA` and `YPO`, by actual distance travelled?

```
start[] <- [['FRA']]
end[] <- [['YPO]]
?[src, dst, distance, path] <~ ShortestPathDijkstra(*route[], start[], end[])
```

| src | dst | distance | path                                                      |
|-----|-----|----------|-----------------------------------------------------------|
| FRA | YPO | 4544.0   | `["FRA","YUL","YVO","YKQ","YMO","YFA","ZKE","YAT","YPO"]` |

CozoDB attempts to provide nice error messages when you make mistakes:

```
?[x, Y] := x = 1, y = x + 1
```

<pre><span style="color: rgb(204, 0, 0);">eval::unbound_symb_in_head</span><span>

  </span><span style="color: rgb(204, 0, 0);">×</span><span> Symbol 'Y' in rule head is unbound
   ╭────
 </span><span style="color: rgba(0, 0, 0, 0.5);">1</span><span> │ ?[x, Y] := x = 1, y = x + 1
   · </span><span style="font-weight: bold; color: rgb(255, 0, 255);">     ─</span><span>
   ╰────
</span><span style="color: rgb(0, 153, 255);">  help: </span><span>Note that symbols occurring only in negated positions are not considered bound
</span></pre>

## Install

This fork targets Rust embedders.

* **Rust library**: add `cozo` to your `Cargo.toml` via the workspace crate in `cozo-core/`.
  Default features are `compact` = `storage-redb` + `requests` + `graph-algo`; that's
  almost certainly what you want.
* **Standalone binary** (`cozo-bin/`): CLI and REPL for ad-hoc queries against a
  redb database file. Build with `cargo build --release -p cozo-bin`.
* **WebAssembly** (`cozo-lib-wasm/`): in-browser build. Currently being rebuilt; don't
  rely on it.

### Backup and restore

There is no in-process `backup_db` / `restore_backup` API. To back up a redb database,
close it and copy the `.redb` file with your usual backup tool (`cp`, `rsync`, `restic`, etc).
To restore, copy the file back. This is a deliberate simplification — redb is a single
file and "copy the file" is a perfectly good backup strategy.

(The old upstream backup mechanism relied on sqlite as an intermediate format;
when sqlite was dropped from this fork, the backup API went with it.)

## Architecture

CozoDB consists of three layers stuck on top of each other,
with each layer only calling into the layer below:

<table>
<tbody>
<tr><td>(<i>User code</i>)</td></tr>
<tr><td>Language/environment wrapper</td></tr>
<tr><td>Query engine</td></tr>
<tr><td>Storage engine</td></tr>
<tr><td>(<i>Operating system</i>)</td></tr>
</tbody>
</table>

### Storage engine

The storage engine defines a `Storage` trait — a key-value interface over binary blobs
with range scan — and two implementations that plug into it: **in-memory** (for tests
and the WASM build) and **redb** (the single persistent backend).

Keys are encoded using a [memcomparable format](https://github.com/facebook/mysql-5.6/wiki/MyRocks-record-format#memcomparable-format)
so that byte-wise lexicographic ordering matches the intended row ordering.

### Query engine

The query engine owns function/aggregation/algorithm definitions, the schema, the
transaction layer, query compilation and execution. Embedders interact with it via
the [Rust API](https://docs.rs/cozo/). The query language reference lives in the
original upstream [execution docs](https://docs.cozodb.org/en/latest/execution.html).

## Status of the project

`cozo-redb` exists so I have a tight, pure-Rust graph database in my personal toolkit.
It is not a community takeover bid, not a claim on the CozoDB name, and makes no
promise of support, release cadence, or stability for anyone else. Pick it up if the
specific shape of "redb + Datalog + time travel + vector search, nothing else" is
what you need.

MPL-2.0, PRs and issues welcome if you are using it.

Versions before 1.0 do not promise syntax/API stability or storage compatibility.

## Links

* [Fork repo](https://github.com/lawless-m/cozo-redb) — this repository
* [Upstream repo (dormant)](https://github.com/cozodb/cozo) — the original CozoDB
* [Upstream query language docs](https://docs.cozodb.org/en/latest/) — tutorial, execution model, built-in functions (mostly accurate for this fork; full-text has been modified and MinHash-LSH removed)
* [Rust API docs](https://docs.rs/cozo/) — generated from upstream's last release; this fork's docs land on docs.rs after a release

## Licensing and contributing

This project is licensed under MPL-2.0 or later.
See [CONTRIBUTING.md](CONTRIBUTING.md) if you are interested in contributing.
