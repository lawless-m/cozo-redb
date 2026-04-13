# A Sermon on Cozo

*For the working programmer who knows SQL, has written Prolog beyond the
child-and-parent examples, and now wishes, with some small impatience, to
understand what cozo actually **is**.*

---

## A word before we begin

Most of the documentation you will find about CozoDB — on its own website,
on GitHub, in the handful of blog posts scattered about the Internet —
announces a list of adjectives (**transactional**, **embedded**, **graph**,
**relational**, **vector**, **Datalog**) and then hurries you onward to the
query reference as though the adjectives were self-explanatory. They are
not. This page is the explanation the cozo lineage has somehow contrived
never to write, and I have taken it upon myself to supply it, because the
alternative is leaving it unwritten forever, which I cannot abide.

You already know SQL. You have written nontrivial Prolog. I shall take
those two facts as fixed points and build outward from them, which means
you will be spared the usual first-principles preamble about what a
relation is and what a rule is. If you have not written Prolog, some of
what follows will still be comprehensible, but you will be reading over
the shoulder of the intended recipient.

---

## What problem is cozo for?

You have facts. Some of those facts are about how things connect — routes
between airports, citations between papers, reports-to lines in a company,
messages between accounts, dependencies between packages, edges in any
graph you care to name. You want to ask questions about those connections:
*what is reachable from here, by how many hops, along the shortest path,
through which intermediary, and how did all of that change on the third
Tuesday of last month?*

You could express these questions in SQL, but the moment the answer
requires recursion — the moment the query involves the phrase "however
many hops it takes" — SQL makes you write `WITH RECURSIVE` ceremonies that
would make a Victorian undertaker proud. You could use a labelled-property
graph database like Neo4j, but then you have committed yourself to a
server process, a foreign query language, and a data model that insists
everything be a node or an edge even when your data is not naturally
shaped that way.

Cozo sits in the gap. It stores ordinary relations (rows in named tables,
just as you know them) and lets you query them in **Datalog**, a
declarative language in which recursion is not a ceremony but the
principal idiom. It runs inside your program's address space (no server,
no socket, no daemon), it commits to a single file via redb (a pure-Rust
mmap B-tree), and it keeps its transactional promises while it does so.

That is the whole pitch, stripped of its buzzwords.

---

## The cast of characters

Before we walk through examples, a short glossary. Each item gets one
sentence; none deserves more here.

* **Stored relation** — a table that persists in redb, has a declared
  schema, and is addressed in queries with a leading `*`, as in `*route`.
* **Derived rule** — a named rule computed during query evaluation, which
  has no existence outside the current query, addressed without the star.
* **Rule** — one line of the form `head := body`, where `head` names an
  output and the body is a conjunction of atoms that bind its variables.
* **Atom** — one named access (`*route{fr: x, to: y}`), one comparison
  (`y > 3`), one binding (`k = x + 1`), or one call into a built-in.
* **`?` rule** — the distinguished output rule of a query; whatever it
  produces is what comes back to the caller.
* **Fixed rule / algorithm** — a built-in operator that runs a packaged
  graph algorithm (`ShortestPathDijkstra`, `PageRank`, and so on) as if it
  were a rule, invoked with `<~` instead of `:=`.
* **Aggregation** — a function that collapses many rows into one value,
  written in the rule head next to the variable it aggregates
  (`count_unique(x)`, `min(x)`, `collect(x)`).
* **Validity** — an optional column type that tags each row with a
  timestamp-and-assert-flag pair, so that the relation records its own
  history rather than overwriting.
* **HNSW index** — a secondary index on a vector column, built via
  `::hnsw create`, used for approximate-nearest-neighbour queries.
* **redb** — the single persistent backend in this fork: a pure-Rust,
  single-file, mmap'd, copy-on-write B-tree with ACID semantics.

---

## If you know Prolog, most of this is already yours

Here is the translation table. Keep it within reach; the rest of the
sermon is essentially commentary upon it.

| Prolog construct | Cozo equivalent | Notes |
|---|---|---|
| `parent(tom, bob).` | `?[p, c] <- [['tom', 'bob']] :put person {p => c}` | A stored fact, but typed, transacted, committed. |
| `parent(X, Y) :- ...` | `parent[x, y] := ...` | `:-` becomes `:=`; the head is `name[args]` not `name(args)`. |
| `?- ancestor(tom, X).` | `?[x] := ancestor['tom', x]` | The `?` rule is the query's output channel. |
| `findall(X, p(X), Bag)` | `?[collect(x)] := p[x]` | Aggregations live in the head. |
| `setof(X, p(X), S)` | `?[collect_unique(x)] := p[x]` | Set semantics are the default; duplicates collapse unless you ask otherwise. |
| `assert(p(1)).` | `?[x] <- [[1]] :put p {x}` | All mutations go through a transaction. |
| `retract(p(1)).` | `?[x] <- [[1]] :rm p {x}` | Symmetrical to `:put`. |
| Cuts, `!` | *(none)* | Datalog has no cut; evaluation is bottom-up and does not backtrack. |
| Lists as `[H\|T]` | *(none)* | Values are scalars, vectors, bytes, strings, or explicit lists — no cons-cell term structure. |
| Compound terms `foo(bar(baz))` | *(none)* | There are no nested function terms; relations carry flat tuples of typed columns. |

The two rows at the bottom of that table are the biggest shift. A great
deal of Prolog idiom depends on cons cells and compound terms — on being
able to pattern-match `tree(L, V, R)` against a recursive data structure —
and Datalog simply does not have those. If you need tree structure, you
store it as a parent-id foreign key in a relation and recurse over rules.

The other large shift is evaluation order. Prolog evaluates top-down with
SLD resolution and backtracking; a carelessly left-recursive rule
(`ancestor(X, Y) :- ancestor(X, Z), parent(Z, Y).`) sends it spinning
into an infinite descent. Datalog evaluates **bottom-up**, in a fixed
point computed by the seminaive algorithm, which means left-recursion is
perfectly fine and in fact the usual way to write transitive closures.
This one fact is, for the Prolog programmer, the single most pleasant
discovery of the afternoon.

---

## The smallest possible query, walked through atom by atom

Here is the shortest non-trivial cozo query:

```
?[x, y] := x = 1, y = x + 1
```

Read it like this:

* `?[x, y]` — the head of the `?` rule. The rule exposes two columns,
  named `x` and `y`, and its rows are whatever the body binds them to.
* `:=` — "is defined as", the separator between head and body.
* `x = 1` — the first atom of the body: an equality that *binds* `x` to
  the value `1`. (There is no notion of "unification of open terms" as in
  Prolog; an equality is a binding or a filter depending on whether the
  variable is already bound.)
* `,` — conjunction; read it as "and".
* `y = x + 1` — a binding again, this time of `y`, computed from the
  already-bound `x`.

Result: one row, `[1, 2]`. The engine has solved a one-row, two-column
query with no I/O.

Add a stored relation to the picture:

```
?[name] := *person{name}
```

* `*person` — a stored relation, introduced earlier via `:create person {name}`
  and populated via `:put`. The leading `*` distinguishes it from a
  derived rule.
* `{name}` — named-column pattern matching. The braces list columns; each
  one names a variable that is bound to that column's value in the
  current row. This is the single biggest syntactic departure from both
  SQL and Prolog, and once you see it you will wonder why SQL never
  bothered.

Result: every name in the `person` relation, one per row.

A slightly richer one, filtering and projecting in one breath:

```
?[name] := *person{name, age}, age >= 18
```

The body binds `name` and `age` from the same row, then applies a filter.
The head projects only `name`. The semantics are set-theoretic: duplicate
rows collapse by default.

That is all the basic syntax you need to read any query in this fork.
Everything after this point is variations on the theme.

---

## Stored versus derived: the distinction that matters

This is the distinction SQL programmers and Prolog programmers both trip
over, though for different reasons.

A **stored relation** is a table. It lives in redb. It has a schema
declared with `:create`, it is mutated with `:put` / `:rm` / `:update`
under a transaction, and it is read with `*name{...}` in rule bodies.
Once committed it persists across process restarts and is visible to
every subsequent transaction.

A **derived rule** is a named expression. It is computed during the
execution of the surrounding query and vanishes when the query ends. It
is defined with `name[args] := body` and read with `name[args]` (no
star). It does not cost storage; it costs compute. You can reuse a
derived rule many times within the same query, and the engine will
compute it once and cache the result.

A SQL programmer's analogy: stored relations are tables, derived rules
are CTEs or views — but views that are computed at query time, not
materialised, and that can refer to themselves.

A Prolog programmer's analogy: stored relations are asserted facts
persisted under a transaction, derived rules are clauses — but clauses
that do not live in a dynamic database and cannot be modified at runtime
except by writing a new query.

The line `?[k] := reachable[k]` reads from a derived rule called
`reachable`; the line `?[k] := *reachable{k}` reads from a *stored
relation* called `reachable`. They are different worlds that happen to
use similar syntax, and keeping them distinct in your head will save you
a great deal of confusion.

---

## Recursion is the whole reason to bother

Here is the airport-reachability example from the README, now walked
through rather than flung at you:

```
reachable[to] := *route{fr: 'FRA', to}
reachable[to] := reachable[stop], *route{fr: stop, to}
?[count_unique(to)] := reachable[to]
```

The first two lines are two clauses of the same rule, `reachable`, which
takes one argument. A rule with more than one clause is the disjunction
of its clauses — read the two lines as "a destination is reachable
either if there is a direct route to it from FRA, **or** if it is
reachable via some intermediate stop that is itself reachable". This is
exactly the pattern you would write in Prolog, with two differences:

1. The engine evaluates the rule **bottom-up**: it first computes all
   direct destinations from FRA, then all destinations reachable via those,
   then all destinations reachable via *those*, and so on until the set
   stops growing. No backtracking, no risk of infinite descent, no need
   to worry about clause order.
2. The result is a **set**. If the same airport is reachable by two
   different paths, it appears once in the answer, not twice.

The third line is the query proper. It reads all rows out of the
derived `reachable` rule and aggregates over them. `count_unique(to)` is
an aggregation that counts distinct values — the engine recognises that
`to` is being aggregated and groups accordingly. The result is a
one-row relation with a single column, which is the count.

Compare what SQL would demand:

```sql
WITH RECURSIVE reachable(to) AS (
    SELECT to FROM route WHERE fr = 'FRA'
    UNION
    SELECT r.to FROM reachable h JOIN route r ON r.fr = h.to
)
SELECT count(DISTINCT to) FROM reachable;
```

...which is technically the same query but with roughly three times the
ceremony, and SQL engines vary in how well they optimise it. Cozo's
engine is built around recursion from the ground up and treats this as
the common case, not a special form.

A Prolog programmer, meanwhile, will recognise the shape of the two
clauses immediately, will notice with relief that the left-recursive
variant `reachable[to] := reachable[stop], *route{fr: stop, to}` does not
loop, and will see that the aggregation has replaced what would have been
a `findall/3` in the calling goal.

---

## Aggregations live in the head

Prolog collects solutions with `findall/3`, `bagof/3`, and `setof/3`. These
are procedurally awkward: you set up a goal, you name a variable to
collect, you get back a list, you process the list. Datalog puts
aggregation syntactically where it belongs — right on the variable being
aggregated, in the rule head:

```
?[count(x)]      := *sales{x}     // "how many sales rows?"
?[count_unique(x)] := *sales{x}   // "how many distinct values of x?"
?[mean(price)]   := *sales{price} // "what is the average?"
?[min(price), max(price)] := *sales{price}
?[country, count(x)] := *sales{x, country}
```

The last one is the most illuminating: the head has one unaggregated
variable (`country`) and one aggregation (`count(x)`). The engine
automatically groups by the unaggregated variables — exactly as SQL's
`GROUP BY` does — and applies the aggregation within each group. No
`GROUP BY` keyword is needed; the presence of an aggregation in the head
implies it.

`collect(x)` and `collect_unique(x)` are the two you will reach for most
often if you come from Prolog: they are exactly `bagof` and `setof`
without the ceremony.

---

## Time travel, concretely

A relation may carry a `Validity` column, which is a pair `[timestamp,
is_assert]`: a microsecond integer plus a boolean that is `true` for
"asserted at this time" and `false` for "retracted at this time". Create
such a relation:

```
:create inventory {sku: String, at: Validity => count: Int}
```

Insert a row "as of now":

```
?[sku, at, count] <- [['widget', 'ASSERT', 100]] :put inventory {sku, at => count}
```

`'ASSERT'` is shorthand for "right now, as an assertion" — the engine
fills in a current timestamp and sets the boolean to `true`. Later, a
second row for the same sku:

```
?[sku, at, count] <- [['widget', 'ASSERT', 80]] :put inventory {sku, at => count}
```

Now the table contains two rows for `widget`. Neither has been overwritten;
the relation has remembered both states. Querying it with `@ "NOW"` asks
for the current view:

```
?[sku, count] := *inventory{sku, count @ "NOW"}
```

...which returns only `widget: 80`. Without the `@`, you see every row in
the history:

```
?[sku, at, count] := *inventory{sku, at, count}
```

...which returns both. And with a literal timestamp in place of `"NOW"`
you can ask what the table looked like on the third Tuesday of last
month. That is the whole of it. It costs storage proportional to history
length and a small query overhead, so you only declare the `Validity`
column on relations that actually need it.

---

## Vector search, concretely

A relation may carry a fixed-dimension vector column, and a **HNSW
index** can be built over it for approximate-nearest-neighbour queries.
Create a relation of 2-dimensional points:

```
:create a {k: String => v: <F32; 2>}
```

Put a few rows in:

```
?[k, v] <- [['a', [1, 2]], ['b', [2, 3]], ['c', [3, 4]]] :put a {k => v}
```

Build the index:

```
::hnsw create a:vec {
    dim: 2,
    m: 50,
    dtype: F32,
    fields: [v],
    distance: L2,
    ef_construction: 20
}
```

Query for the two nearest neighbours of a given point:

```
?[dist, k] := ~a:vec{k | query: q, k: 2, ef: 20, bind_distance: dist},
              q = vec([2, 2])
```

The `~a:vec` form is the vector-query atom. It binds `k` from each hit,
binds `dist` from the named parameter `bind_distance`, and takes the
query vector `q` and the search parameters as named arguments. The rest
of the query can then filter, join, or aggregate over the results like
any other relation. In production you would more likely build 1536-dim
embeddings from a neural network and query them with semantic questions,
but the shape of the thing is what matters here.

---

## Full-text search, concretely

Text columns can be covered by a **full-text index** backed by tantivy.
The index is declared after the relation and updated automatically as
the relation is mutated. Create a relation of notes:

```
:create notes {id: Int => title: String, body: String}
```

Declare the index, naming the text columns to cover:

```
::fts create notes:ft { fields: [title, body] }
```

Populate the relation in the usual way, with `:put`:

```
?[id, title, body] <- [
    [1, 'rust graph notes', 'a short note about rust and graphs'],
    [2, 'python notes',     'nothing about rust here'],
    [3, 'rustacean diary',  'mostly about graphs, some about rust']
] :put notes {id => title, body}
```

And query the index with a `~rel:ft{...}` atom, structurally the twin of
the HNSW atom:

```
?[id, score] := ~notes:ft{id | query: "+rust graph", k: 10, bind_score: score}
```

The `query` string is passed verbatim to tantivy's query parser, which
understands `+must`, `-must_not`, phrase queries in quotes, field-scoped
searches, and the rest of the usual text-search vocabulary. `k` caps the
number of hits returned; `bind_score` exposes tantivy's relevance score
so you can order or filter by it; the columns listed before the pipe
(`id` here) are bound from the base relation for each hit, exactly as
with `~name:vec{...}` atoms.

A small caution: this fork's FTS differs from upstream cozo's. Upstream
had its own hand-rolled indexer; this fork replaced it with tantivy. The
atom shape is the same, but the query-language grammar inside the
`query` string is whatever tantivy's parser understands, which is
[documented by the tantivy project](https://docs.rs/tantivy/latest/tantivy/query/struct.QueryParser.html)
rather than in the upstream cozo tutorial. The upstream pages on
full-text search are no longer accurate here.

---

## Why redb?

Upstream cozo once offered five persistent backends and spent a good deal
of its code mediating between them. This fork ships exactly one, because
on the published benchmarks redb won or tied on every read and
aggregation workload, beating sqlite by 32–49% across the board and
finishing time-travel aggregation over a million-row relation 2.35×
faster than its nearest competitor. It is also the only backend that was
*pure Rust*, single-file, and free of any C++ submodule — which matters
rather a lot if you intend to build for WASM, or for a target that cannot
easily carry a vendored `librocksdb`, or for embedded hardware where a
42 MB C++ dependency is embarrassing. It is the only backend the author
of this fork had any interest in maintaining, and so it is the only
backend the fork has.

---

## Where this sermon ends

What this page does not cover:

* **The full query reference** — every built-in function, every
  aggregation, every option to `:create`, the exact grammar, the full
  list of HNSW tuning parameters. That lives in the upstream
  [execution docs](https://docs.cozodb.org/en/latest/execution.html)
  and the [tutorial](https://docs.cozodb.org/en/latest/tutorial.html),
  which are still largely accurate for this fork. The modified parts
  (full-text search, removed MinHash-LSH, removed non-Rust bindings)
  are catalogued in [DIFFERENCES.md](DIFFERENCES.md).
* **The Rust embedding API** — how to call `Db::run_script`, how to
  construct a `NamedRows`, how to register fixed rules from Rust. That
  lives in the [Rust API docs on docs.rs](https://docs.rs/cozo/), bearing
  in mind that the published version is upstream's; this fork's docs
  land on docs.rs only after a release.
* **Operational matters** — backup (covered in `DIFFERENCES.md`:
  `.redb` is a single file, so `cp` is your backup tool), performance
  tuning, sharding (there is none — this is a single-node embedded
  engine), and high availability (likewise).

What I hope you carry away from it: Cozo is Datalog with a relational
storage engine, a first-class recursion facility, and a transactional
persistent backend. If you know Prolog, you already know how to read a
rule, what recursion buys you, and why aggregations in the head are a
tidier arrangement than `findall`. If you know SQL, you already know how
to read a relation and a filter and a group-by, and you already suspect
that `WITH RECURSIVE` should not require quite so much effort. Cozo is
what you would have built if you had had time: the good parts of SQL,
the good parts of Prolog, and none of the accidental ceremony of either.

The rest is practice.
