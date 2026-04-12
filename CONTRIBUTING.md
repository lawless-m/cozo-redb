# CONTRIBUTING, or, A Prospectus for Players

## Preamble, Delivered from the Apron of the Stage

My dear sir — or madam, as the case may be — or any other person who
finds themself with pen in hand, spirit in foot, and a wandering gaze
drawn toward the playbill of this small but (I trust) not undistinguished
provincial company: you are most cordially welcomed to the boards of
`cozo-redb`. We are a touring troupe of modest ambitions and a single
fixed engagement — namely, a tight, Rust-first graph database built upon
redb — and we are always, my dear sir, _always_ in want of talent.

Set down below are the terms upon which a person may, should he be so
disposed, secure himself a part in the ongoing production.

## The Profession, and How One Joins It

There are, broadly speaking, two ways of walking on in this company:

1. **A walking-on part** — a small bug-fix, a typographical mending,
   a patch of the most modest ambition. In such cases, pray dispense
   with the preliminaries altogether. Open a
   [pull request](https://github.com/lawless-m/cozo-redb/pulls), with a
   plain description of what ails the text and how your amendment shall
   cure it, and the thing shall be considered directly.

2. **A speaking part** — a new feature, a refactor of any substance, a
   reshaping of an interface, or any contribution that shall require the
   rearranging of scenery. For these I must insist upon an audition
   *before* rehearsal: open a
   [Discussion](https://github.com/lawless-m/cozo-redb/discussions) or
   an [Issue](https://github.com/lawless-m/cozo-redb/issues) and let us
   first agree upon the shape of the business. It spares everyone —
   yourself most of all — the humiliation of a rehearsed scene the
   manager had no intention of ever staging.

Bug reports, likewise, belong in the
[Issues](https://github.com/lawless-m/cozo-redb/issues) register; ideas,
questions, and the general taking of counsel belong in the
[Discussions](https://github.com/lawless-m/cozo-redb/discussions).

## The Rehearsal Room

Before the dress rehearsal can be called, the piece must first be
rehearsed in private. Pray, at your own bench:

- `cargo build` — let the thing stand up.
- `cargo test` — let the thing not fall over.
- `cargo clippy -- -D warnings` — let the thing wear its costume cleanly.
- `cargo fmt --all` — let the thing be properly dressed.

If any of these falls short, mend it before you come to the first night.
The dress rehearsal shall be no kinder than the manager himself.

## The Dress Rehearsal

This is conducted by the theatrical machinery itself — the Continuous
Integration workflow in `.github/workflows/`. It shall rehearse the piece
upon its own apparatus and report to the company. Should the curtain fall
upon a red light, it is your task, my dear sir, to investigate the cause
and put it right. The stagehands do not come running.

## The Audition, or, The Review

Every pull request shall be read, and read plainly, by the maintainer of
this fork. In accordance with the
[Code of Conduct](CODE_OF_CONDUCT.md) set out elsewhere in this
repository, you may expect honest speech and no fine-spun apology. If the
piece wants rewriting, you shall be told so; if it wants striking out
altogether, you shall be told that too. Receive the review as the old
players did: with thanks, and with a quick pencil.

## Of Properties and Scenery

Mind, when you would add a new dependency to the `Cargo.toml`, that this
is a company of narrow ambitions — **Rust-first, redb-only, nothing else**.
New crates are not forbidden, but each one shall be weighed upon the
scales of necessity before being permitted upon the stage. A contribution
that drags a dozen new trunks of scenery into the wings for the sake of
a single line of business is unlikely to be received with enthusiasm.

## Licensing, Briefly

This fork is licensed under the **Mozilla Public License 2.0**, in common
with the upstream from which it descends. Contributors need sign no CLA,
no DCO, no contract in blood — the act of opening a pull request is taken
as an indication that the contributor is content for their work to be
distributed under the same licence. If you are not content with this,
pray do not open the pull request.

## A Closing Word from the Manager

This is a small company, my dear sir, but an earnest one, and we have no
grand expectations of the wider world. Bring us what you have; we shall
read it; we shall tell you plainly what we think of it; and if it is
good, the curtain shall rise upon it. Should the piece prove, upon
further reflection, unsuited to our humble playbill — why, there are
other theatres, and no hard feelings between players.

And remember, as old Mr Crummles was wont to say: there's nothing like
the profession.

— The Management
