# vidya-on-oxigraph PRD

Supersedes `docs/prd-vidya.md` for the storage layer, tool surface, and crate structure. The reasoning engine (derive, analyze) has been extracted to panini — this PRD covers vidya as a pure knowledge store.

## Problem Statement

Vidya's current Postgres-backed storage model creates two problems:

1. **Deployment friction.** Panini needs to ship as a single binary. The current architecture requires a running Postgres instance, a separate vidya systemd service, and MCP communication between them. Users shouldn't need to install and configure Postgres to use a Sanskrit grammar tool.

2. **Impedance mismatch.** Vidya's data is a knowledge graph — entities, relationships, and provenance annotations on facts. The relational schema (8 tables, JSONB columns, multi-table joins for provenance) forces graph-shaped queries through a relational mold. Claims, assertions, and entities are all triples wearing a relational costume.

## Solution

Rewrite vidya on Oxigraph, an embeddable RDF triplestore in pure Rust. This gives:

- **Embeddable storage** — panini compiles vidya-core as a library dependency, opens the store in-process. Single binary, no sidecar services.
- **Native graph model** — facts are triples, provenance is RDF-star annotations on triples, domains are named graphs. No impedance mismatch.
- **Standard query language** — SPARQL internally (not exposed to MCP consumers), replacing hand-built SQL.
- **Simplified codebase** — the current 2,100 lines of Postgres-shaped code (db.rs, query.rs, load.rs, claim.rs, relation.rs) are replaced by a smaller Oxigraph-native implementation.

## User Stories

1. As an agent, I want to describe an entity by name within a domain, so that I get all its properties and relationships with provenance in one call.
2. As an agent, I want to search for entities by type and attribute filters within a domain, so that I can find entities matching specific criteria (e.g., "all grahas with element=fire").
3. As an agent, I want to traverse relationships from an entity to a configurable depth, so that I can walk the knowledge graph (e.g., friends-of-friends).
4. As an agent, I want to query the provenance of a specific fact, so that I can report which tradition, source, and pramana support it and with what confidence.
5. As an agent, I want to filter any query by tradition and/or pramana, so that I can scope answers to a specific epistemological perspective.
6. As an agent, I want to assert a new fact with required provenance, so that knowledge discovered mid-session is recorded with full epistemological metadata.
7. As an agent, I want to bulk-load a domain from a Turtle file, so that curated ontologies can be loaded in one operation.
8. As an agent, I want to bulk-load a domain by file path, so that large seed files don't need to be passed inline through MCP.
9. As a developer, I want to embed vidya as a Rust library in panini, so that panini ships as a single binary with in-process knowledge access.
10. As a developer, I want vidya's base ontology (Tradition, Source, Pramana, Assertion) to load automatically on store initialization, so that every domain can reference shared epistemological concepts without boilerplate.
11. As a developer, I want domain data isolated in named graphs, so that domains don't leak into each other but cross-domain queries are possible when needed.
12. As a developer, I want the six pramana types as first-class RDF resources, so that they can carry properties and participate in queries rather than being opaque strings.
13. As a developer, I want seed data authored as Turtle files, so that the source-of-truth format matches the storage model.
14. As a developer, I want vidya to run as an MCP server (systemd service) for agent access, with the same query logic available as a library for embedded access.

## Implementation Decisions

### Crate structure

Two crates in a Cargo workspace:

- **vidya-core** (library) — `KnowledgeStore` wrapper, query methods, assertion logic, Turtle loading, ontology model. No MCP dependency, no async runtime required. This is what panini depends on.
- **vidya** (binary) — thin MCP server using rmcp. Deserializes tool parameters, delegates to vidya-core methods, serializes JSON responses.

### Storage model

- Single Oxigraph store backed by RocksDB, located at `~/.vidya/store/`.
- Each domain is a named graph (e.g., `<http://vidya.ninthhouse.studio/domain/jyotish/>`).
- The base `vidya:` ontology lives in the default graph, loaded automatically on store initialization.
- Cross-domain queries are opt-in — queries scope to a named graph by default.

### Base ontology (`vidya:`)

Defined in `ontology/vidya.ttl`, embedded in the binary or loaded from disk.

Classes:
- `vidya:Tradition` — a lineage or school. Property: `vidya:parentTradition` for hierarchy.
- `vidya:Source` — a text, teacher, or reference. Properties: `vidya:sourceKind` (text, oral, commentary), `vidya:reliability` (xsd:float).
- `vidya:Assertion` — provenance bundle linking a triple to its epistemological metadata.
- `vidya:Pramana` — the category of valid knowledge.

Pramana instances (six, per the classical tradition):
- `vidya:pratyaksha` (perception)
- `vidya:anumana` (inference)
- `vidya:shabda` (authoritative testimony)
- `vidya:upamana` (analogy)
- `vidya:arthapatti` (presumption)
- `vidya:anupalabdhi` (non-apprehension)

Properties on assertions:
- `vidya:assertedBy` — RDF-star annotation linking a triple to its Assertion node
- `vidya:tradition` — Assertion → Tradition
- `vidya:source` — Assertion → Source
- `vidya:pramana` — Assertion → Pramana
- `vidya:confidence` — Assertion → xsd:float

### MCP tool surface (4 tools)

| Tool | Purpose |
|---|---|
| `vidya_health` | Store connectivity, triple/graph counts, version |
| `vidya_load` | Bulk load a domain. Params: `domain` + `turtle` (inline string) or `path` (file path). Loads into a named graph. |
| `vidya_assert` | Assert a single triple with required provenance. Params: `domain`, `subject`, `predicate`, `object` (short names, resolved to IRIs by domain prefix), `literal` (bool, defaults true — set false for entity references), `provenance` object (`tradition`, `source`, `pramana`, `confidence`). |
| `vidya_query` | Structured query with four modes and optional cross-cutting filters. |

### vidya_query modes

**describe** — all triples about a subject, with provenance. Params: `domain`, `subject`.

**search** — find entities by type and attribute filters. Params: `domain`, `kind`, optional `filter` (attribute key-value pairs).

**traverse** — walk relationships from a subject. Params: `domain`, `subject`, `predicate`, `depth`.

**provenance** — epistemological metadata for a specific triple. Params: `domain`, `subject`, `predicate`, `object`.

Cross-cutting optional filters on all modes: `tradition`, `pramana`. These scope results to facts asserted by a specific tradition or known through a specific pramana type.

### Query interface

Structured parameters only — agents never write SPARQL. Vidya builds SPARQL internally from the structured query shapes. If raw SPARQL access is needed later, it belongs in the library API (vidya-core), not the MCP surface.

### IRI resolution

Short names in MCP tool parameters are resolved to full IRIs using the domain prefix. `"surya"` in domain `"jyotish"` resolves to `<http://vidya.ninthhouse.studio/domain/jyotish/surya>`. Predicates are resolved the same way. Base ontology terms (e.g., pramana names, `assertedBy`) resolve against the `vidya:` prefix.

### Seed data

- `ontology/vidya.ttl` — base vocabulary, versioned with the vidya crate.
- `seeds/jyotish.ttl` — jyotish domain data, converted one-time from `seeds/jyotish.json`.
- `seeds/vyakarana.ttl` — vyakarana domain data, converted one-time from `seeds/vyakarana.json`.

After conversion, Turtle is the source-of-truth format. JSON seeds can be archived or deleted.

### Modules

1. **vidya-core::store** — `KnowledgeStore` wrapper around Oxigraph `Store`. Opens/creates the store, auto-loads base ontology on init, manages named graphs. Interface: `open(path)`, `open_read_only(path)`, `load_domain(name, turtle)`, `load_domain_from_file(name, path)`.

2. **vidya-core::ontology** — IRI constants, prefix resolution (short name → full IRI given domain), pramana type constants. Owns `ontology/vidya.ttl` (embedded via `include_str!` or loaded from disk).

3. **vidya-core::query** — the deep module. Four query modes as methods on `KnowledgeStore`. Each builds SPARQL internally, executes against the store, returns structured Rust types. Cross-cutting tradition/pramana filters applied here.

4. **vidya-core::assert** — triple assertion with provenance. Resolves short names to IRIs, constructs RDF-star annotated triples with Assertion blank nodes, inserts into the named graph.

5. **vidya MCP layer** — thin binary crate. rmcp tool handlers that deserialize params, call vidya-core, serialize JSON responses.

6. **Seed conversion** — one-time script to transform JSON seeds to Turtle. Not a permanent part of the crate.

## Testing Decisions

### What makes a good test

Tests validate external behavior through vidya-core's public API, not internal SPARQL generation. A test loads known Turtle data, calls a query/assert method, and asserts on the returned Rust types.

### Modules under test

- **vidya-core::query** — the deepest module and most likely to have subtle bugs. Test each query mode: load a known Turtle file, run describe/search/traverse/provenance, verify results. Test cross-cutting filters (tradition, pramana). Test edge cases: empty results, unknown subjects, multi-hop traversal.
- **vidya-core::assert** — assert a triple with provenance, then query it back via describe and provenance modes. Verify the round-trip: subject/predicate/object present, provenance metadata intact.
- **vidya-core::store** — test initialization (base ontology auto-loaded), domain loading (triples land in correct named graph), open read-only mode.

### Prior art

The existing `tests/integration.rs` (3,000 lines) provides the pattern: load seed data, run operations, verify results against a real store. The new tests will follow the same pattern but operate against an in-memory Oxigraph store (via `Store::new()`) rather than Postgres.

## Out of Scope

- **Reasoning engine** — derive, analyze, engine strategies. Extracted to panini.
- **Raw SPARQL in MCP** — structured params only for now. Add a `sparql` escape hatch later if needed.
- **Retraction/deletion tool** — add when there's a use case.
- **Declarative domain DSL on top of Turtle** — Turtle is expressive enough for current needs.
- **Migration from Postgres** — this is a rewrite, not a migration. No compatibility layer.
- **Web UI or product layer** — vidya is infrastructure.

## Further Notes

### Oxigraph version and API

Oxigraph 0.5.8 (April 2026) is in active development. Key API notes:
- `store.query()` is deprecated — use `SparqlEvaluator::new().parse_query(sparql)?.on_store(&store).execute()?`
- `rdf-12` feature flag required for RDF-star (`<<subject predicate object>>` syntax)
- Cargo.toml already has `oxigraph = { version = "0.5", features = ["rocksdb", "rdf-12"] }`
- Reference the spike code (`examples/oxigraph-spike.rs`) and crate source for API patterns.

### Relation to panini deployment

The vidya-core / vidya split is driven by panini's deployment model: single binary, no external services. Panini compiles vidya-core as a cargo dependency, opens the Oxigraph store in-process via `KnowledgeStore::open_read_only(path)`. The MCP server (vidya binary) is for agent access; the library (vidya-core) is for embedded access. Same query logic, two access patterns.
