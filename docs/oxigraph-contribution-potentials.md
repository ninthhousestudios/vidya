# Oxigraph contribution potentials

Written 2026-05-22. Notes on what contributing to oxigraph would look
like, as both a way to derisk the vidya dependency and a vehicle for
learning Rust more deeply.

Source: github.com/oxigraph/oxigraph (cloned at ../oxigraph).

## Issue reassessment

The earlier risk summary overstated some bugs. On closer inspection:

- **#279 (DELETE WHERE misses triples):** Fixed in PR #296 (Nov 2022).
  Was a datetime encoding issue. Issue stayed open because conversation
  drifted to JS bindings. Not a current risk.

- **#646 (GROUP BY + HAVING returns nothing):** Not a bug. SPARQL spec
  says HAVING doesn't have access to SELECT projections — you must
  repeat the aggregate expression. Oxigraph follows the spec strictly;
  other stores like Jena are lenient. Closed by maintainer.

- **#487 (block checksum mismatch):** Real, but triggers at ~138M
  triples during bulk load. RocksDB SST file writer issue. Conversation
  went cold. Not relevant at vidya's current or near-term scale.

## Codebase architecture

~84K lines of Rust, ~17 crates. Key layers:

```
oxrdf (data model)
  → oxttl (Turtle parser, ~9,500 LoC)
  → oxrdfio (format dispatch)
spargebra (SPARQL parser/AST, ~9,700 LoC)
  → sparopt (optimizer, ~3,200 LoC)
  → spareval (evaluator, ~9,700 LoC — eval.rs alone is 4,694)
oxigraph (Store + public API, storage layer ~7,500 LoC)
  → oxrocksdb-sys (RocksDB FFI)
```

Tests: inline `#[cfg(test)]` modules, integration tests in `tests/`
dirs, and a dedicated W3C conformance testsuite crate (`testsuite/`)
that runs official SPARQL and RDF test suites. The testsuite is the
safety net — any change can be checked against spec compliance.

No CONTRIBUTING.md. CI is thorough: clippy with `-D warnings`, cross-
compilation to WASM and 32-bit, daily fuzz runs via `sparql-smith`.

## Contribution tiers

### Tier 1 — Tests and docs

**Effort:** weeks. **Rust learning:** moderate.

Write regression tests for reported edge cases, add examples, improve
documentation. The project has no CONTRIBUTING.md and sparse docs.
Low barrier to entry. Good for learning the codebase structure and
Rust testing patterns without needing to understand the internals
deeply.

### Tier 2 — Parser and serializer bugs

**Effort:** weeks to months. **Rust learning:** high.

The Turtle parser (`oxttl`) and SPARQL parser (`spargebra`) are
self-contained crates. Parser bugs are usually reproducible with a
specific input string, and fixes tend to be localized. Parsers are
pattern-matching code — good Rust learning territory (enums, iterators,
error handling, lifetime management in streaming parsers).

This is where Claude can help most effectively — reading parser code,
tracing a specific input through the state machine, identifying where
the parse diverges from the grammar.

### Tier 3 — Query evaluator bugs

**Effort:** months. **Rust learning:** very high.

The heart of oxigraph: `spareval/src/eval.rs` (4,694 lines). A tree-
walking interpreter over SPARQL algebra nodes. Understanding why a
query returns wrong results requires understanding how variable bindings
flow through joins, filters, aggregation, and subqueries.

Claude can read and explain this code, but building intuition for the
evaluator's behavior takes sustained exposure. The W3C testsuite helps
— you can write a failing test from a bug report and then trace through
eval.rs to find where the result diverges.

Real bugs in this tier tend to involve subtle interactions: variable
scoping across subqueries, binding propagation through OPTIONAL/UNION,
aggregate evaluation order.

### Tier 4 — Storage and RocksDB

**Effort:** large. **Rust learning:** moderate (mostly FFI and unsafe).

The #487 checksum bug lives here. The storage layer wraps RocksDB via
`oxrocksdb-sys` (vendored C++ with Rust FFI). Debugging requires
understanding RocksDB internals — compaction, SST files, checksumming.
This is where even the maintainer doesn't have answers.

Probably not worth pursuing unless a storage bug directly blocks vidya.
The Rust learning value is narrow (FFI patterns, unsafe blocks) and
the domain knowledge (RocksDB) doesn't transfer broadly.

## As a Rust learning vehicle

The codebase covers a wide range of Rust patterns:

- **Enums and pattern matching:** the RDF data model (oxrdf) and SPARQL
  AST (spargebra) are heavily enum-based
- **Iterators and streaming:** parsers process input as streams, the
  evaluator produces lazy result iterators
- **Error handling:** thiserror, custom error types, Result chains
- **Traits and generics:** the store abstracts over memory/RocksDB
  backends
- **Lifetimes:** the evaluator and transaction types have nontrivial
  lifetime bounds
- **FFI:** oxrocksdb-sys wraps C++ via bindgen
- **Testing:** property-based fuzzing (sparql-smith), W3C conformance,
  integration tests

Tiers 1-2 cover the first four well. Tier 3 adds lifetimes and
iterator complexity. Tier 4 adds FFI.

## Relationship to vidya

Contributing to oxigraph derisks vidya's dependency: deeper familiarity
with the codebase means faster response to breaking changes, better
ability to assess whether an upgrade is safe, and ultimately the option
of maintaining a fork if needed.

It's also seva in a different direction — the Rust RDF ecosystem is
thin and oxigraph is effectively it. Improvements benefit everyone
building on RDF in Rust.

The tradeoff is time: oxigraph contributions are a detour from ayus
and vidya feature work. Probably best pursued as a background learning
activity rather than a primary workstream.
