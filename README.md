[![CI](https://github.com/lawless-m/cozo-redb/actions/workflows/build.yml/badge.svg)](https://github.com/lawless-m/cozo-redb/actions/workflows/build.yml)
[![License](https://img.shields.io/github/license/lawless-m/cozo-redb)](https://github.com/lawless-m/cozo-redb/blob/main/LICENSE.txt)

# `cozo-redb`

> **A Rust graph database backed by [redb](https://github.com/cberner/redb).**
>
> `cozo-redb` is a Rust-first fork of [cozodb/cozo](https://github.com/cozodb/cozo) —
> Ziyang Hu's transactional Datalog database. It keeps the query language, Datalog
> semantics, time-travel relations, and HNSW vector search, and uses **redb**
> (a pure-Rust mmap B-tree) as its single persistent backend.
>
> This fork ships only the `mem` and `redb` backends; rocksdb, sled, sqlite, tikv, and
> the non-Rust language bindings are gone. For other notes, see
> [DIFFERENCES.md](DIFFERENCES.md).
>
> If you have never met Cozo before and would like a walked-through explanation — what
> Datalog is, how rules and stored relations fit together, why recursion is the whole
> point, and what time travel, vector search, and full-text search look like in the
> concrete — read [SERMON.md](SERMON.md). It is the in-depth introduction the cozo
> lineage never quite got around to writing.
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

`cozo-redb` is — and I always say this to anyone who will listen, though
there are fewer of them these days than one might wish — an embedded,
transactional database, which is only a fine way of saying that it lives
quietly in the same room as the program it serves, and does not go
shouting at it from another house altogether. It is queried in **Datalog**
(a language, I am assured by those who know about such things, of
considerable cleverness and long pedigree), and it keeps relational,
graph, and vector data all under the one roof — which I always think is
much the most sensible arrangement, for I cannot abide a household in
which everything is scattered about in different rooms. It remembers,
too, what it used to know — _time travel_, they call it, though it is
nothing more alarming than a well-kept diary — and it stores all its
affairs in a single tidy file by means of **redb**, which is, I am told,
a pure-Rust mmap B-tree; and whilst I could not pretend to explain what
that is, I am quite sure it is a very respectable thing indeed.

### Embedded only

A Rust crate that runs in your program's process. No server, no socket,
no daemon, no port to open. Add `cozo` to your `Cargo.toml`, call it
from your code, ship one binary.

### Why graphs

Most interesting questions about data are questions about relationships:
who connects to whom, what leads to what, how far apart two things are.
SQL can express them, but recursive traversals are awkward and slow.
`cozo-redb` stores data in ordinary relations and queries them with
Datalog, which handles recursion natively — shortest-path, reachability,
and PageRank are one query, not twenty.

This is **not** a labelled-property graph database. There are no
nodes-and-edges primitives. Model your data as relations; the graph is
whatever the relations describe.

### Why Datalog

Queries are composed from named rules, not nested subqueries. Recursion
is first-class — a rule may refer to itself — so graph traversals and
transitive closures are written directly, without `WITH RECURSIVE`
gymnastics. Anything SQL can express, Datalog can express, usually more
cleanly.

### Time travel

Every relation _may_ track its own history. Updates don't overwrite —
they append a new version tagged with a validity time. Queries can then
ask "what did this relation look like last Tuesday?" and get the
Tuesday answer.

It costs storage and a little query overhead, so it is **opt-in per
relation**. If a relation doesn't need history, don't enable it.

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
* **WebAssembly** (`cozo-lib-wasm/`): in-browser build via `wasm-pack`, exposing
  a `CozoDb` JS class backed by `MemStorage`. Full-text search works in the
  browser too, via an in-RAM tantivy index. See `cozo-lib-wasm/build.sh` —
  cross-compiling the `zstd-sys` shim requires a wasm-capable clang
  (`apt install clang-19` on Debian).

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
MPL-2.0, PRs and issues welcome if you are using it.

Versions before 1.0 do not promise syntax/API stability or storage compatibility.
For the fork's scope and what has been trimmed from upstream, see
[DIFFERENCES.md](DIFFERENCES.md).

## Links

* [Fork repo](https://github.com/lawless-m/cozo-redb) — this repository
* [Upstream repo (dormant)](https://github.com/cozodb/cozo) — the original CozoDB
* [Upstream query language docs](https://docs.cozodb.org/en/latest/) — tutorial, execution model, built-in functions (mostly accurate for this fork; full-text has been modified and MinHash-LSH removed)
* [Rust API docs](https://docs.rs/cozo/) — generated from upstream's last release; this fork's docs land on docs.rs after a release

## Licensing and contributing

This project is licensed under MPL-2.0 or later.
See [CONTRIBUTING.md](CONTRIBUTING.md) if you are interested in contributing.
