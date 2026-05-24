# vidya

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)

Vidya is a structured knowledge graph for domain reasoning with
provenance. It gives LLM agents access to cited, tradition-aware facts
instead of relying on training data or flat RAG retrieval.

The Sanskrit *vidyā* (विद्या) means knowledge or learning — specifically
the kind that comes from study and discipline, not from casual
familiarity.

Part of [manas](https://github.com/ninthhousestudios/manas/), a modular
agent infrastructure built in Rust.

## The problem vidya solves

LLMs are unreliable in domains where:

- **Traditions disagree.** Vedic and Western astrology share vocabulary
  but diverge on substance. An LLM blends them into plausible-sounding
  output that is technically wrong — the worst kind of wrong, because you
  need domain expertise to catch it.
- **Knowledge is structured, not textual.** "Saturn is exalted in Libra"
  is a fact with provenance, not a passage to retrieve. RAG asks the LLM
  to reconstruct a rule system from prose on every query.
- **Relationships are dense.** Rulerships, aspects, dignities, mutual
  receptions — this is a graph. Flattening it to text and hoping the LLM
  reconstructs the structure is fragile.

Vidya stores domain knowledge as an RDF graph with typed relationships,
explicit tradition scoping, and provenance on every assertion.

## Architecture

### Oxigraph + RDF-star

Vidya is backed by [Oxigraph](https://github.com/oxigraph/oxigraph), an
embedded RDF triple store. Each domain lives in its own named graph.
SPARQL is used internally; agents interact through structured query
parameters, not raw SPARQL.

Provenance uses RDF-star annotations — every significant triple can
carry metadata about who asserted it, from which tradition, citing which
source, with what confidence:

```turtle
<< jyotish:surya jyotish:exaltedIn jyotish:mesha >>
    jyotish:exaltationDegree 10 ;
    vidya:assertedBy [
        vidya:tradition  jyotish:tradition-bphs ;
        vidya:source     jyotish:source-bphs ;
        vidya:pramana    vidya:shabda ;
        vidya:confidence "1.0"^^xsd:float
    ] .
```

### Base ontology

The base ontology (`ontology/vidya.ttl`) defines cross-domain concepts:

- **Tradition** — a lineage or school (e.g. Parashara, Jaimini)
- **Source** — a specific text with a reliability score
- **Pramana** — means of knowledge, modeled as six first-class resources
  from Indian epistemology: pratyaksha (perception), anumana (inference),
  shabda (authoritative testimony), upamana (analogy), arthapatti
  (presumption), anupalabdhi (non-apprehension)

Domain-specific classes and properties are declared in each domain's
seed file.

### Natural language resolution

When a query doesn't match an exact entity name, vidya falls back to NL
resolution. It builds a vocabulary index from the loaded domain
(entity names, aliases, labels, type names, predicates, property values)
and matches input tokens through a cascade: exact match, substring,
edit distance, then VSA similarity.

The VSA (Vector Symbolic Architecture) layer uses Holographic Reduced
Representations (HRR) to encode each entity as a high-dimensional
vector composed from its relationships. This enables fuzzy matching
based on structural similarity — two entities with overlapping
properties will have similar vectors even if their names are unrelated.

The vocabulary and VSA index are cached per domain and invalidated
automatically when domain data is reloaded.

### Crate structure

- **vidya-core** — library crate: `KnowledgeStore`, query engine,
  ontology loading. Embeddable by other Rust projects.
- **vidya** — binary crate: CLI + MCP server over Streamable HTTP with
  auth-token gating.

## CLI

The CLI lets humans (and agents via shell) query the knowledge graph
directly without an MCP server running.

### Loading domains

```
vidya load jyotish seeds/jyotish.ttl
vidya load ayurveda seeds/ayurveda.ttl
vidya domains
```

Domains persist in the Oxigraph store (`~/.vidya/store/`). Load once,
query indefinitely.

### Querying

All query commands take `-d <domain>` or read from the `VIDYA_DOMAIN`
env var. Add `--json` for machine-readable output.

```
# Describe an entity — properties and provenance
vidya describe -d jyotish surya

# Search by type, with optional attribute filters
vidya search -d jyotish Graha
vidya search -d jyotish Graha -f element=fire

# Walk relationships
vidya traverse -d jyotish surya naturalFriend --depth 2

# Epistemological metadata for a specific triple
vidya provenance -d jyotish surya exaltedIn mesha
```

Cross-cutting filters narrow results by tradition or pramana:

```
vidya search -d jyotish Graha --tradition tradition-bphs
vidya describe -d jyotish surya --pramana vidya:shabda
```

Set `VIDYA_DOMAIN` to skip the `-d` flag when working in one domain:

```
export VIDYA_DOMAIN=jyotish
vidya describe surya
vidya search Graha -f element=fire
```

### Natural language resolution

All query commands accept natural language input as a fallback. Exact
names and structured flags always work as before — NL resolution only
activates when the structured path returns NotFound.

```
# Western names resolve to domain entities
vidya describe -d jyotish mars        # → mangala
vidya traverse -d jyotish sun rules   # → surya rules

# Free-text search with type + filter
vidya search -d jyotish "fire graha"      # → element=fire filter on Graha
vidya search -d jyotish "malefic graha"   # → nature=malefic filter on Graha

# Typo correction
vidya describe -d jyotish mangla      # → mangala (edit distance 1)
```

When NL resolution activates, vidya prints what it resolved to stderr:

```
  resolved: subject: mangala
```

Resolution uses a four-stage cascade: exact match, substring match,
edit distance, then VSA similarity. See
[docs/natural-language-queries.md](docs/natural-language-queries.md) for
the full set of examples and limitations.

### Store access

Query commands open the store read-only, so they work while the systemd
service holds the write lock. Only `vidya load` requires exclusive
(read-write) access — stop the service first if it's running.

## MCP tools

| Tool | Purpose |
|------|---------|
| `vidya_health` | Status, triple count, loaded domains |
| `vidya_load` | Load a domain from inline Turtle or a `.ttl` file path |
| `vidya_query` | Query in four modes: **describe** (entity profile), **search** (find by kind + filters), **traverse** (walk relationships), **provenance** (epistemological metadata for a triple). Names can be exact domain terms or natural-language aliases. |
| `vidya_assert` | Assert a single triple with required provenance |

Cross-cutting filters on `tradition` and `pramana` apply to all query
modes.

### Response format

`vidya_query` returns a JSON envelope:

```json
{
  "result": { ... },
  "resolution": {
    "details": ["subject: mangala"],
    "unknown_tokens": []
  }
}
```

The `result` field contains the query data. The `resolution` field is
present only when NL fallback was used — exact-hit responses omit it.
This gives clients a consistent shape to parse regardless of whether
resolution was needed.

## Domains

### Jyotish (Vedic astrology) — active

The jyotish seed (`seeds/jyotish.ttl`, ~1,029 triples) covers:

- 9 grahas with attributes, aliases, karakas
- 12 rashis, 12 bhavas, 4 dignity types
- Planetary dignities, friendships, aspects
- 3 traditions, 4 sources
- RDF-star provenance on all relational claims, including contested
  assertions (e.g. Rahu/Ketu dignities at confidence 0.7)

### Ayurveda — active

Dravyaguna (pharmacology) covering substances with rasa, guna, veerya,
vipaka, karma properties sourced from Charaka, Sushruta, and
Bhavaprakasha — especially where they diverge. Seed data in
`seeds/ayurveda.ttl`.

## What fits in vidya (and what doesn't)

Vidya earns its keep for domains where the same question has legitimately
different answers depending on who you ask, and tracking that matters.
Provenance, multi-tradition perspectives, confidence-weighted assertions.

It is not a good fit for:
- Procedural rules (computational transformations, grammar engines)
- General-purpose notes or personal knowledge (that's kosha, maybe)
- Data that changes frequently (the seed model is batch-oriented)

## Deployment

Runs as a systemd user service:

```
~/.cargo/bin/vidya serve --http --auth-token-file ~/.vidya/auth-token
```

- Default port: 3300
- Store path: `~/.vidya/store/` (Oxigraph persistent storage)
- Auth: bearer token from `~/.vidya/auth-token`
- Transport: Streamable HTTP (MCP 2025-03-26)

## Relationship to other manas subsystems

- **chitta** holds the person model (preferences, values, patterns).
  Vidya holds domain knowledge — what is known, not who knows it.
- **smriti** indexes files. Vidya structures knowledge above the file
  level.
- **sutra** provides code intelligence. Vidya provides domain
  intelligence.

