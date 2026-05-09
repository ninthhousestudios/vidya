# vidya

[![License: MPL 2.0](https://img.shields.io/badge/License-MPL_2.0-brightgreen.svg)](https://opensource.org/licenses/MPL-2.0)

Vidya is a structured knowledge graph for domain reasoning with
provenance. It gives LLM agents access to cited, tradition-aware facts
instead of relying on training data or flat RAG retrieval.

The Sanskrit *vidyā* (विद्या) means knowledge or learning — specifically
the kind that comes from study and discipline, not from casual
familiarity.

Part of [manas](https://github.com/ninthhousestudios/manas/), a modular
agent infrastructure built in Rust.

## The problem vidya solves

Pure RAG (retrieve chunks, feed to LLM) breaks down in domains where:

- **Traditions disagree.** Vedic and Western astrology share vocabulary
  but diverge on substance. Embedding similarity retrieves chunks that
  *sound* similar; the LLM blends them into plausible-sounding output
  that is technically wrong. The worst kind of wrong — you need domain
  expertise to catch it.
- **Knowledge is rule-based, not textual.** "Saturn is exalted in Libra"
  is a structured fact, not a passage. RAG asks the LLM to reconstruct a
  rule system from retrieved prose on every query.
- **Relationships are dense.** Rulerships, aspects, dignities, mutual
  receptions — this is a graph. RAG flattens it to text and hopes the LLM
  can reconstruct the structure on demand.

Vidya addresses this by storing domain knowledge as structured claims with
typed relationships, explicit tradition scoping, and provenance links back
to source material in kosha (document comprehension) or canonical texts.

## Design

### Entities, claims, and relations

The core model:

- **Entities** — things in a domain (Saturn, Libra, the 10th house).
- **Claims** — statements about entities ("Saturn is exalted in Libra"),
  each scoped to a tradition and backed by at least one source citation.
- **Relations** — typed edges between entities and claims
  (rules, exalts, aspects, contradicts, refines, derives from).

Claims are immutable once accepted. Corrections are new claims that
supersede via derivation. Displaced claims become `historical` rather
than being mutated or deleted.

### Traditions as a first-class concept

Every claim is scoped to a tradition (Vedic, classical Western,
Hellenistic, KP, etc.). This prevents the blending problem: when an
agent asks "what are Saturn's dignities?", vidya can return
tradition-specific answers rather than a confused merge.

### Provenance

Every claim requires at least one source — a kosha chunk ID (linking to
the exact passage in a book), a tradition reference, or a practitioner
self-citation. The agent can follow provenance to quote the original
material.

### Schema sketch

```
domains(id, slug, title)
entities(id, domain_id, name, kind, metadata)
claims(id, domain_id, statement, confidence, status)
sources(id, kind, ref)
traditions(id, domain_id, name)
relations(id, src_type, src_id, dst_type, dst_id, kind, metadata)
```

Postgres with foreign keys. No RDF/SPARQL — the tooling tax doesn't pay
back without OWL reasoning needs.

### Claim lifecycle

```
proposed  →  active  →  historical
                ↑            ↑
            (review)    (superseded by new claim)
```

Three extraction sources, all gated by human review:

- **Foundational claims** — hand-curated from canonical texts with
  citation. Tedious but bounded.
- **LLM-assisted extraction** — LLM proposes claims from kosha chunks;
  practitioner reviews and accepts. Faster, lower precision, gated.
- **Practitioner knowledge** — claims from accumulated expertise not in
  any one book. Source is self-citation.

## Why astrology first

Most domains (medicine, law) have unbounded canons — extraction never
ends. Astrology has a finite foundational rule set: all dignities, all
aspects, all houses, all dashas, all yogas, planetary characteristics.
Thousands of claims, not millions. You can plausibly *finish* the
foundational extraction, then depth comes from interpretation (LLM + RAG
over kosha) on top of structured facts.

## Relationship to other manas subsystems

- **kosha** provides the source material — book chunks, document
  embeddings. Vidya claims cite kosha chunk IDs for provenance.
- **chitta** holds the person model. A `josh_holds` relation can
  cross-link a vidya claim to a chitta memory, capturing the
  practitioner's personal stance on a contested point.
- **smriti** handles file-level perception. Kosha handles content-level
  perception. Vidya sits above both as the structured knowledge layer.

## Status

Vidya is in early design. The schema, extraction pipeline, and MCP tool
surface are defined in
[`docs/knowledge-stack.md`](https://github.com/ninthhousestudios/manas/blob/main/docs/knowledge-stack.md)
but no code has been written yet. The plan is to start with a single
small domain (~100 hand-curated claims) to prove the model before
scaling extraction.

## Planned deployment

Same pattern as chitta and smriti:

- **vidya-engine** — Rust crate, no I/O surface
- **vidya-server** — thin MCP wrapper over HTTP
- Astrological domains loaded for aion (professional astrology tool)
- Software/methodology domains loaded for manas dev workflows
- Both deployments cite kosha for provenance

## License

MPL-2.0.
