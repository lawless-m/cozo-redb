<img src="static/logo_c.png" width="200" height="175" alt="Logo">

[![CI](https://github.com/lawless-m/cozo-rs/actions/workflows/build.yml/badge.svg)](https://github.com/lawless-m/cozo-rs/actions/workflows/build.yml)
[![License](https://img.shields.io/github/license/lawless-m/cozo-rs)](https://github.com/lawless-m/cozo-rs/blob/main/LICENSE.txt)

# `CozoDB` — maintained fork

> **You probably arrived here looking for [cozodb/cozo](https://github.com/cozodb/cozo).**
> That project has been dormant since December 2024. This is a personal maintenance fork,
> not affiliated with the original author. The **query language, semantics, and most of
> the core engine are unchanged** — so the [upstream documentation](https://docs.cozodb.org/)
> and its many mirrors (readthedocs, docs.rs for published crates, etc.) still describe
> how queries work here. What's different:
>
> * added a `redb` storage backend (with time travel),
> * retired the `cozorocks` C++ FFI subcrate in favour of the pure-Rust `rocksdb` crate,
> * dropped the `sled` backend (read perf regressed 6× between smoke and medium scale in benchmarks; unreliable for production),
> * carries benchmark infrastructure and comprehensive CI,
> * Rust-only — the Python, Node, Java, Clojure, Go, Swift, Android and C bindings from
>   upstream are not maintained here; if you need those, upstream v0.7 still works.
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
>
> We say CozoDB is _embeddable_ instead of _embedded_ since you can also use it in client-server
> mode, which can make better use of server resources and allow much more concurrency than
> in embedded mode.

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

CozoDB targets embedded, single-box workloads. Recent backend benchmarks from this fork
(`cozo-core/BENCHMARKS.md`) show redb beating sqlite on every read and aggregation workload
by 32–49%, with time-travel aggregation over a 1M-row relation 2.35× faster. Write throughput
is within ~6% between redb and sqlite at 1M rows. The pure-Rust rocksdb backend is roughly
on par with redb for reads and 1.8–2.3× faster on writes than the retired `cozorocks` FFI
build (the gain is pure FFI elimination). See `cozo-core/BENCHMARKS.md` for the numbers.

## Getting started

The query language reference (tutorial, execution model, built-in functions) is still hosted
at the original upstream docs site — this fork has not replicated it. Start with the
[tutorial](https://docs.cozodb.org/en/latest/tutorial.html), then see the
[execution model](https://docs.cozodb.org/en/latest/execution.html). Everything there still
applies to this fork's query engine.

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

This fork targets Rust embedders first.

* **Rust library**: add `cozo` to your `Cargo.toml` via the workspace crate in `cozo-core/`.
  Enable the backends you want via features: `storage-sqlite`, `storage-rocksdb`,
  `storage-redb`. The `graph-algo` feature pulls in the built-in graph algorithms.
* **Standalone binary** (`cozo-bin/`): HTTP server + CLI for ad-hoc queries against a
  database file. Build with `cargo build --release -p cozo-bin --features compact,storage-rocksdb`.
* **WebAssembly** (`cozo-lib-wasm/`): in-browser build. Currently being rebuilt; don't
  rely on it.

Other language bindings (Python, Node, Java, Clojure, Go, Swift, Android, C) existed
in upstream cozo and have been removed from this fork. If you need one of those, look
at the upstream repository — the bindings there still work against upstream v0.7 and
can be adapted to this fork's query engine if anyone wants to maintain them.

### Storage backends

| Feature flag        | Backend       | Notes                                                                                                     |
|---------------------|---------------|-----------------------------------------------------------------------------------------------------------|
| (always)            | In-memory     | Non-persistent. Fastest. Used by tests and the WASM build.                                                |
| `storage-sqlite`    | SQLite        | Historical default. Also used as the backup/interchange format.                                           |
| `storage-rocksdb`   | RocksDB       | Pure-Rust via the `rocksdb` crate. Highest write throughput and concurrency.                              |
| `storage-redb`      | redb          | Pure-Rust, mmap B-tree. Wins all read and aggregation workloads vs sqlite (see `cozo-core/BENCHMARKS.md`). Recommended default. |

### Tuning the RocksDB backend

The RocksDB backend ships with sensible defaults (9.9-bits-per-key bloom filter with
whole-key filtering, 9-byte prefix extractor matching CozoDB's key layout) which are
fine for 95% of workloads.

External RocksDB options-file loading (the "drop an `options` file in the data directory"
escape hatch from upstream cozo) is not currently wired up against the pure-Rust backend.
If CozoDB finds an `options` file in your data directory it logs a warning and falls back
to the built-in tuning. Re-enabling this is tracked as future work.

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
with range scan — and several implementations that plug into it: in-memory, SQLite,
RocksDB, and redb. A TiKV client backend exists in the tree but is not exercised
in this fork. Rust embedders can also provide custom backends by implementing the trait.

The SQLite backend also doubles as the backup/interchange file format, so you can
move data between backends by round-tripping through a SQLite export.

Keys are encoded using a [memcomparable format](https://github.com/facebook/mysql-5.6/wiki/MyRocks-record-format#memcomparable-format)
so that byte-wise lexicographic ordering matches the intended row ordering. This is
why a SQLite backend file can't be usefully queried with regular SQL — the blobs are
opaque without CozoDB's decoder.

### Query engine

The query engine owns function/aggregation/algorithm definitions, the schema, the
transaction layer, query compilation and execution. Embedders interact with it via
the [Rust API](https://docs.rs/cozo/). The query language reference lives in the
original upstream [execution docs](https://docs.cozodb.org/en/latest/execution.html).

## Status of the project

This is a personal maintenance fork of [cozodb/cozo](https://github.com/cozodb/cozo),
which has been dormant since December 2024. It exists so I have a non-Neo4j graph
database in my toolkit that I can actually keep alive: adding backends, fixing bugs,
wiring up CI, and following interesting changes from other forks as they surface.

It is not a community takeover bid and makes no promise of support, release cadence,
or stability for anyone else. That said, the code is MPL-2.0 and PRs / issues are
welcome if you're using it.

Versions before 1.0 do not promise syntax/API stability or storage compatibility.

## Links

* [Fork repo](https://github.com/lawless-m/cozo-rs) — this repository
* [Upstream repo (dormant)](https://github.com/cozodb/cozo) — the original CozoDB
* [Upstream query language docs](https://docs.cozodb.org/en/latest/) — tutorial, execution model, built-in functions (still accurate for this fork)
* [Rust API docs](https://docs.rs/cozo/) — generated from upstream's last release; this fork's docs land on docs.rs after a release

## Licensing and contributing

This project is licensed under MPL-2.0 or later.
See [CONTRIBUTING.md](CONTRIBUTING.md) if you are interested in contributing.
