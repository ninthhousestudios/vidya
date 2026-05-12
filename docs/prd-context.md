# vidya PRD context

Context dump from the 2026-05-10 brainstorming and spike session.
Use this as input when writing the PRD — not as the PRD itself.

## What vidya is

A structured knowledge graph with reasoning. Three-layer model:

1. **Ontology** — the grammar of a domain. Entity kinds, relation kinds,
   claim templates. Domain-configurable. Small (dozens of entries).
2. **Facts** — instances. Entities, claims (structured, not text blobs),
   relations. The bulk content.
3. **Epistemology** — provenance + pramāṇa. Every claim has assertions
   scoped to a tradition, backed by a source, tagged with how the
   knowledge was established (pratyakṣa/anumāna/śabda/upamāna/
   arthāpatti/anupalabdhi). Derivation chains link conclusions to
   premises.

Claims are immutable once accepted. Corrections are new claims that
supersede via derivation. Status: proposed → active → historical.

## What vidya is not

- Not a document store (that's kosha)
- Not a memory system (that's chitta)
- Not an embedding/vector search tool — vidya is structured queries,
  not similarity search
- Not a product — products are built *on top of* vidya

## The spike (2026-05-10)

Built and validated in a single session:

- Postgres schema: 11 tables across three layers, migrations applied
- 7 MCP tools: vidya_health, vidya_domain, vidya_entity, vidya_claim,
  vidya_query, vidya_load, vidya_derive
- Bulk loader with transactional, idempotent seeding from JSON files
- Forward-chaining derivation engine with domain-specific strategies
- Two domains loaded:
  - **vyākaraṇa** (Pāṇinian Sanskrit grammar): 44 entities, 32 claims,
    21 sandhi rules. Derivation engine correctly applies all tested
    vowel sandhi (savarṇa-dīrgha, guṇa, vṛddhi, yaṇ) with sutra
    references in traces.
  - **jyotiṣa** (Vedic astrology): 37 entities, 79 claims, 37
    relations. Complete dignity table, planetary natures, aspects,
    kārakas, natural friendships. All sourced from BPHS.
- 10/10 sandhi derivation tests pass

Spike code: `/home/josh/soft/manas/vidya/` (branch: main, commit from
session). Schema is solid and should carry forward. Code quality is
spike-level.

## Domain reasoning tiers

Not all domains need the same thing from vidya:

| Tier | What it needs | Example |
|---|---|---|
| Query-only | Entities + claims + structured queries | Geology, taxonomy |
| Rule-application | Generative rules, derivation engine | Pāṇini grammar, type systems |
| Interpretive | Structured facts as spine, LLM does inference | Astrological interpretation |

## Domain-specific reasoning: current and future

Currently: compiled Rust strategy modules in `engine/`. One module per
operation type per domain (`engine/sandhi.rs`). Requires recompilation
to add new domains.

Future direction (not yet designed, revisit with 3+ domains):
**Declarative rule semantics in claim templates.** The template's
`param_schema` carries enough metadata for a generic engine to interpret
rules without domain-specific code. Analogous to Pāṇini's paribhāṣā
(meta-rules about how rules work). Design this after the Pāṇini product
has 2+ working engine strategies (sandhi + declension) to extract the
pattern from.

Plugin alternatives considered and deferred:
- Embedded scripting (Rhai/Lua/WASM)
- Compiled dynamic libraries (dlopen)

## The Pāṇini product (separate from vidya)

### Architecture

- Separate project, separate repo, separate identity
- Uses vidya as knowledge backend via MCP
- Adds Sanskrit-specific orchestration:
  - Paradigm generation (call vidya_derive per vibhakti+vacana combo)
  - Sandhi splitting (generate all breakpoints, check against known rules)
  - Word analysis (given inflected form, trace back to stem + suffixes)
- Web UI for students + MCP tools for agents
- Pedagogical sequencing informed by Ruppel's *Cambridge Introduction
  to Sanskrit*

### What the product needs from vidya

1. **More sandhi rules.** Current: vowel sandhi only (21 rules).
   Needed: consonant sandhi (visarga, final stops), which is where
   most complexity lives.
2. **Declension engine strategy.** New operation in vidya_derive:
   given a stem type and vibhakti/vacana, apply suffix-attachment and
   sandhi rules to produce the inflected form with trace.
3. **Verb conjugation strategy.** Given a dhātu and lakāra, produce
   conjugation table.
4. **Sandhi splitting (reverse derivation).** Given a combined form,
   enumerate possible decompositions that match known sandhi rules.
5. **Dhātupāṭha as seed data.** The verb root list with meanings and
   gaṇa assignments.
6. **More sūtras encoded.** Current: 7 sūtras as entities. The
   Aṣṭādhyāyī has ~4000. Encode progressively, not all at once.

### Pedagogical direction

Ruppel's *Cambridge Introduction to Sanskrit* sequences:
1. Sandhi (vowel, then consonant, then visarga)
2. Noun declension (a-stems, then ā-stems, then i/u stems, then
   consonant stems)
3. Verb conjugation (present tense parasmaipada, then ātmanepada,
   then other tenses)
4. Compounds
5. Participles and secondary derivation

This is also a natural priority order for which rules to encode in
vidya first.

### User experience vision

The core value: **show *why* a form looks the way it does**, not just
*what* the form is. Derivation traces with sūtra citations at every
step. When a student encounters "devānām" in a text:

```
devānām = deva + ām (genitive plural, a-stem masculine)
  step 1: deva + ām → apply 6.1.101 (savarṇa-dīrgha) → devānām
  source: Aṣṭ. 6.1.101 (akaḥ savarṇe dīrghaḥ)
  tradition: pāṇini
  pramāṇa: śabda
```

For sandhi splitting, show all valid decompositions with confidence:
```
tasyeṣṭam =
  1. tasya + iṣṭam (guṇa sandhi, 6.1.87) ← most likely
  2. [no other valid decompositions]
```

## Pramāṇa model

Six classical pramāṇas, mapped to practical epistemology:

| Pramāṇa | Meaning | Use in vidya |
|---|---|---|
| pratyakṣa | Direct observation | Corpus evidence, experimental results |
| anumāna | Inference | Claims derived by reasoning from other claims |
| śabda | Authoritative testimony | Canonical texts, textbook assertions |
| upamāna | Analogy | Knowledge by structural comparison |
| arthāpatti | Postulation | Logically entailed by other facts |
| anupalabdhi | Non-apprehension | Knowledge from absence |

Operationally useful: an agent should reason differently depending on
pramāṇa type. Śabda claims are only as reliable as their source.
Anumāna claims can be checked by tracing the derivation. Pratyakṣa
claims update with new observations.

## Embeddings

Vidya does not use embeddings. Knowledge is structured, queries are
structural. The LLM translates natural language to structured queries.
Entity aliases (in attrs) handle the "Saturn" → "Śani" mapping.
Embedding-based search is kosha's domain.

## Schema (validated)

```sql
-- Ontology: domains, entity_kinds, relation_kinds, claim_templates
-- Facts: entities, claims, relations
-- Epistemology: traditions, sources, assertions, derivations
```

Full migration: `migrations/0001_schema.sql`

Key design choices:
- UUIDs (v7, time-sortable) everywhere
- Claims deduped on `(domain_id, template_id, md5(params::text))`
- Traditions are hierarchical (parent_id)
- Sources have reliability scores
- Assertions carry pramāṇa + confidence

## Stack

- Rust (edition 2024)
- Postgres (no pgvector — vidya is structural, not semantic)
- rmcp for MCP server (stdio + streamable HTTP)
- sqlx for DB access with embedded migrations
- axum/tower for HTTP transport
- Seed data as JSON files in `seeds/`

## Open design questions for the PRD

- **Seed data authoring workflow.** Hand-editing JSON is fine for a
  spike. What's the curation workflow for 500+ sūtras? CLI tool?
  Web form? LLM-assisted extraction from texts?
- **Versioning.** When a domain's rules change (e.g., you fix an
  incorrectly encoded sūtra), what happens to derivations that used the
  old version? Immutable claims help, but the derivation trace points at
  a specific claim ID.
- **Cross-domain queries.** Can the Pāṇini product query both vyākaraṇa
  and jyotiṣa domains simultaneously? (Probably not needed, but the
  schema supports it.)
- **Performance.** Forward chaining loads all rules into memory. Fine
  for 21 sandhi rules, maybe not for 4000 sūtras. May need indexing or
  caching on the rule-matching step.
- **Deployment model.** Vidya as a systemd user service (like chitta)?
  Docker? Both?
- **Testing strategy.** The Sanskrit language itself is the test suite —
  every correctly derived form validates the rules, every incorrect form
  is a bug report. How to encode this as automated tests?
- **Collaboration with domain experts.** Ruppel or other Sanskritists
  as advisors on rule encoding and pedagogical design?
    yes...i want to make an mvp and then reach out to her and show it.

## References

- Spike code: this repo (`/home/josh/soft/manas/vidya/`)
- Manas architecture: `~/soft/manas/docs/manas-architecture.md`
- Knowledge stack: `~/soft/manas/docs/knowledge-stack.md`
- Ruppel, *The Cambridge Introduction to Sanskrit* (pedagogical reference)
- Pāṇini, *Aṣṭādhyāyī* (primary source for vyākaraṇa domain)
- *Bṛhat Parāśara Horā Śāstra* (primary source for jyotiṣa domain)
