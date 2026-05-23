# RDF, SPARQL, and the oxigraph dependency

Written 2026-05-22. Explains the technology stack vidya is built on, why
it was chosen, the risks of the current implementation, and contingency
paths.

## The stack, bottom up

### RDF — the data model

Everything in vidya is triples: subject–predicate–object.

    turmeric  hasRasa     katu
    turmeric  hasVeerya   ushna
    turmeric  commonName  "turmeric"

No tables, no fixed schema. You assert facts as triples. A dravya has
rasa, guna, karma. A planet has dignity, aspect, house. A Sanskrit word
has declension, gender, root. All in the same graph, all linkable.

You don't design tables per domain — you describe the shape of knowledge
in an ontology (classes and properties) and pour facts in. New domains
don't require migrations; they require new ontologies and new TTL files.

### TTL (Turtle) — the serialization format

Turtle is the human-readable syntax for RDF triples:

    ayurveda:haridra  a ayurveda:Dravya ;
        rdfs:label           "haridra" ;
        ayurveda:commonName  "turmeric" ;
        ayurveda:hasRasa     ayurveda:katu, ayurveda:tikta ;
        ayurveda:hasVeerya   ayurveda:ushna .

Vidya uses TTL as both the authoring/interchange format (seed files,
extraction pipeline output) and the load format at runtime.

### Ontologies — domain descriptions

An ontology declares the classes and properties for a domain. Vidya's
ayurveda ontology defines `Dravya`, `Rasa`, `Guna`, `Dosha`, `Karma` as
classes, and `hasRasa`, `hasGuna`, `pacifiesDosha`, etc. as properties
with domain/range constraints. See `seeds/ayurveda.ttl`.

Ontologies are themselves RDF — classes and properties are triples too.
This means the schema is queryable the same way the data is.

### SPARQL — the query language

SPARQL is pattern matching over triples. The relationship of SPARQL to
RDF is like SQL to relational databases.

    SELECT ?dravya ?name WHERE {
      ?dravya a ayurveda:Dravya .
      ?dravya ayurveda:hasRasa ayurveda:katu .
      ?dravya ayurveda:commonName ?name .
    }

This says: "find things that are Dravyas, have katu rasa, and return
their names." The engine matches patterns across the triple graph. You
can traverse arbitrarily — "find dravyas that pacify the same dosha that
this guna aggravates" — without knowing the traversal path at design
time.

For the "any knowledge" vision, this matters: if you don't know what
domains vidya will hold, you don't know what queries people will need.
SPARQL lets you ask questions you didn't anticipate. A custom query
layer only handles the patterns you coded.

### RDF-star (RDF 1.2) — provenance annotations

RDF-star lets you make statements about statements:

    << turmeric hasRasa katu >> citedIn "Charaka SS 27.15"

"The fact that turmeric has katu rasa is cited in verse 27.15." This is
how vidya tracks provenance — every fact can carry annotations about
where it came from. Vidya uses the oxigraph `rdf-12` feature for this.

### Named graphs — domain isolation

RDF supports named graphs: a set of triples identified by a URI. Vidya
uses these to isolate domains — the ayurveda graph is separate from a
jyotish graph. Queries can target one graph or span all of them.

## Why this stack was chosen for vidya

The original vidya was Postgres-backed (relational schema with JSONB
columns). The rewrite to oxigraph was motivated by two problems, documented
in `docs/prd-vidya-oxigraph.md`:

1. **Deployment friction.** Vidya needed to embed into single-binary
   applications (panini, ayus) without requiring a running Postgres
   instance.

2. **Impedance mismatch.** Knowledge graph data (entities, relationships,
   provenance on facts) was being forced through a relational model.
   8 tables, multi-table joins for provenance — triples in a relational
   costume.

RDF+SPARQL+oxigraph solved both: embeddable Rust library, native graph
model, standard query language.

The "any knowledge" generalization is the deeper reason. RDF is one of
the most defensible choices for a universal knowledge representation.
The semantic web ecosystem got a lot wrong about adoption, but the data
model is sound for a service that holds arbitrary domain knowledge.

## Oxigraph specifically

Oxigraph is an embeddable RDF triplestore written in Rust by Thomas
Pellissier Tanon. It is the only mature-ish option in the Rust RDF
space. Repository: github.com/oxigraph/oxigraph.

### Current state (as of May 2026)

- Version: 0.5.8 (released April 2026)
- License: Apache 2.0
- Has never reached 1.0 in 7 years of development
- README states: "Oxigraph is in heavy development and SPARQL query
  evaluation has not been optimized yet."

### Strengths

- Active development — monthly patches in 0.5.x throughout 2026
- Embeddable (RocksDB-backed or in-memory)
- Supports RDF 1.2 (RDF-star), which vidya requires for provenance
- Clean Rust API
- SPARQL compliance is reasonably good for common query patterns

### Risk factors

**Bus factor = 1.** Thomas has 2,064 of ~2,174 total commits. The next
human contributor has 25. If he stops, the project stalls.

**Breaking API changes at every minor version.** The 0.4→0.5 transition
removed `QueryOptions`, `QueryResults`, `Subject`, restructured the
evaluator API. Even within 0.5 betas, lifetime bounds changed and
types were deleted.

**Correctness bugs open for years:**
- #279 (2022): DELETE WHERE misses triples — open 4 years
- #487 (2023): block checksum mismatch on large bulk loads — open 3 years
- #950 (2024): duplicate blank nodes in query results
- #1450 (2025): wrong scoping of graph name variables
- #646 (2023): GROUP BY with HAVING returns no results — open 2.5 years

**SPARQL performance explicitly unoptimized.** Fine for small datasets
(hundreds of entities). Concerning if vidya scales to large knowledge
bases.

**No Rust alternative.** Other crates in the space:
- sophia — lower-level RDF library, no built-in SPARQL store
- rio — Turtle/N-Triples parser only, no query engine
- nemo — rule/reasoning engine, not a triplestore
- None of these are drop-in replacements

## Vidya's coupling to oxigraph

All oxigraph usage is in 3 files in `vidya-core/src/`:

**store.rs** — `Store` (open/create), `RdfParser`/`RdfFormat` (TTL
loading), `NamedNodeRef`/`NamedOrBlankNode` (IRI handling),
`SparqlEvaluator`/`QueryResults` (query execution).

**query.rs** — `NamedNodeRef`, `Term` (result matching),
`SparqlEvaluator`/`QueryResults` (query execution). SPARQL queries are
built as strings via vidya's own `SparqlBuilder`, not oxigraph APIs.

**error.rs** — `QueryEvaluationError`, `LoaderError`, `StorageError`
mapped via `#[from]` into `VidyaError`.

The application layer (MCP server, main binary, consumers like ayus)
has zero direct oxigraph imports. Everything goes through
`KnowledgeStore`.

**Leaks in the abstraction:**
- `KnowledgeStore::inner()` exposes the raw `Store` (used in tests)
- `VidyaError` has `#[from]` on three oxigraph error types

**RDF-star is the deepest lock-in.** All provenance queries use `<< s p o >>`
annotation syntax. Any replacement backend must support RDF 1.2, or the
provenance model needs redesigning.

Total SPARQL execution sites: ~7 (3 in describe, 1 each in search/
traverse/provenance, 1 in ontology check).

## Contingency paths

### Do nothing (current plan for MVP)

Pin to `oxigraph = "0.5"`. Don't upgrade casually. The correctness bugs
surface on large bulk loads and complex query patterns — for small
datasets (ayus with 83 dravyas) the risk is low. Revisit when vidya's
usage outgrows the current constraints.

### Tighten the abstraction (low cost, high optionality)

Remove `KnowledgeStore::inner()`. Wrap all SPARQL execution in a single
function. Replace `#[from]` error conversions with explicit mappings.
This costs an afternoon and makes either migration path below
mechanical rather than archaeological.

### Migrate to a simpler store

If vidya's query patterns remain known and finite (describe, search,
traverse, provenance), a full SPARQL engine is overkill. The same
queries could be implemented over SQLite with a triples table, or over
an in-memory graph structure, using pattern-matching code instead of
SPARQL strings.

- Keep TTL as the authoring format (parse with rio or sophia)
- Implement the finite query patterns directly
- Lose the ability to ask arbitrary SPARQL questions
- Gain a dependency you fully control with no correctness risk

This is a real redesign of vidya-core internals — not a weekend swap,
but the coupling surface is small enough to be tractable.

### Wrap a mature non-Rust store

Apache Jena is the gold standard: Java-based, 20+ years old, rock
solid, full SPARQL 1.1 and RDF-star support. Vidya-core could talk to
Jena as a local service (HTTP SPARQL endpoint) instead of embedding
oxigraph.

- Preserves full SPARQL generality and the "any knowledge" vision
- Proven correctness and performance at scale
- Loses the single-binary deployment model (Jena needs a JVM)
- Could be offered as an alternative backend alongside embedded oxigraph

### Maintenance fork of oxigraph

If Thomas stops maintaining oxigraph, forking for maintenance-only
work (keep it building, security patches, compatibility bumps) is
feasible. Active stewardship (fixing correctness bugs, optimizing the
query engine, evolving the API) is a much larger commitment — the
SPARQL spec is enormous and the query evaluation engine is where all
the hard bugs live.

## Recommendation

For the MVP and near-term: do nothing, pin the version, build the demo.

As a cheap hedge: tighten the abstraction boundary in vidya-core. This
is worth doing regardless of which path the future takes.

Long-term decisions should wait until vidya's actual usage patterns,
scale requirements, and the state of the Rust RDF ecosystem are clearer.
The architecture (RDF data model, TTL authoring, ontology-driven
domains, provenance annotations) is sound independent of which engine
executes the queries.
