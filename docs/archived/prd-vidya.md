# vidya PRD

## Problem Statement

LLM agents need access to structured domain knowledge — facts, rules, relationships, and their provenance — in a form they can query and reason over. Current approaches fail in predictable ways:

- **RAG** blends traditions that disagree (Vedic vs. Western astrology share vocabulary but diverge on substance), loses structure (Saturn's exaltation in Libra is a structured fact, not a passage to retrieve), and can't represent dense relational graphs.
- **Training data** is uncitable, unversioned, and unscoped. An agent can't say *where* it learned something or *which tradition* it's drawing from.
- **Hardcoded logic** (compiled per-rule modules) doesn't scale. You can write a Rust function for 21 sandhi rules, not for 4000 sūtras.

Domains that combine structured facts with generative rules (Pāṇinian grammar, type systems, formal ontologies) need an engine that stores knowledge *and* reasons over it, with full provenance at every step.

## Solution

Vidya is a structured knowledge graph with domain-specific reasoning, provenance tracking, and an MCP interface for agent consumption. It provides:

1. **A three-layer knowledge model** — ontology (domain grammar), facts (instances), epistemology (provenance) — that separates what you know from how you know it.
2. **A derivation engine** that applies rules stored as knowledge (not code) to produce new facts with traced reasoning, in both forward and reverse directions.
3. **An MCP tool surface** that lets agents query, derive, and analyze knowledge without understanding vidya's internals.

Vidya is not a product — products are built on top of it. It is not a document store (that's kosha), not a memory system (that's chitta), and not an embedding/vector search tool. Vidya is structural queries and rule-based reasoning over cited, tradition-scoped knowledge.

## User Stories

The primary users are LLM agents and the developers who configure vidya for their domains.

1. As an agent, I want to query entities by kind and attributes within a domain, so that I can retrieve structured facts without loading everything client-side.
2. As an agent, I want to query claims for a specific entity filtered by tradition, so that I can give tradition-accurate answers.
3. As an agent, I want to traverse relations from an entity with configurable depth, so that I can walk knowledge graphs (e.g., "what is this word's synonym cluster, and what ontological class does the cluster belong to?").
4. As an agent, I want to call forward derivation with an input and get back a result with a step-by-step trace citing the rules applied, so that I can show users *why* a form looks the way it does.
5. As an agent, I want to call reverse analysis with a surface form and get back ranked candidate decompositions, so that I can perform sandhi splitting and word analysis.
6. As an agent, I want to query the derivation chain for any claim back to its original sources, so that I can cite provenance to users.
7. As an agent, I want to load bulk seed data via MCP with idempotent behavior, so that I can curate knowledge programmatically and self-correct on errors.
8. As an agent, I want clear error messages from vidya_load when my seed data violates the ontology (wrong param shape, nonexistent entity kind, etc.), so that I can fix issues without human intervention.
9. As a developer, I want to define a new domain by specifying entity kinds, relation kinds, and claim templates, so that vidya can host any structured knowledge domain.
10. As a developer, I want to register an engine strategy (Rust trait impl) for a domain that needs reasoning, so that derivation and analysis work for that domain's rules.
11. As a developer, I want query-only domains (no engine strategy) to work out of the box, so that domains like jyotiṣa that only need structured storage and retrieval require no custom code.
12. As a developer, I want cross-domain relations to work within a knowledge cluster, so that a kosha domain can reference entities in the vyākaraṇa domain.
13. As a developer, I want to run vidya as a systemd user service with a fixed MCP endpoint, so that multiple agents can connect concurrently.
14. As a developer, I want a test harness that validates derivation results against expected outputs per claim template type, so that the knowledge itself serves as the test suite.

## Implementation Decisions

### Three-layer schema

Carries forward from the spike with additions:

- **Ontology layer** — `domains`, `entity_kinds`, `relation_kinds`, `claim_templates`. Domain-configurable, small (dozens of entries per domain). `claim_templates.param_schema` is a JSON Schema that is validated at load time, not just documentation.
- **Fact layer** — `entities`, `claims`, `relations`. Claims are immutable once active (status: `proposed → active → historical`). Corrections are new claims that supersede via derivation. Relations can reference entities from any domain within the knowledge cluster.
- **Epistemology layer** — `traditions` (hierarchical), `sources` (with reliability scores), `assertions` (tradition + source + pramāṇa + confidence), `derivations` (conclusion ← premise chain with step ordering).

### Schema additions for rule metadata

Claim params for rule-bearing templates (sandhi_rule, declension_rule, etc.) carry machine-interpretable fields beyond the rule content itself:

- `sutra_position` — adhyāya.pāda.sūtra as a sortable string (e.g., "06.01.101"), enabling positional ordering
- `rule_type` — one of: `utsarga` (general), `apavāda` (exception), `nitya` (obligatory), `paribhāṣā` (meta-rule), `tripādi` (late-pass)

These live in the claim params (not new columns) because they're domain-specific — only vyākaraṇa needs them. Other domains define whatever params their templates require.

### Engine strategy trait

Domain-specific reasoning is provided by Rust trait implementations:

```
trait EngineStrategy:
    fn can_handle(&self, domain: &str, operation: &str) -> bool
    async fn derive(&self, pool, request) -> Result<DeriveResult>
    async fn analyze(&self, pool, request) -> Result<Vec<AnalysisCandidate>>
```

- `derive` — forward: given input, apply matching rules, produce result with trace
- `analyze` — reverse: given a form, enumerate valid decompositions with the rule that produced each

Domains that need reasoning register a strategy. Query-only domains register nothing and use `vidya_query` exclusively.

**Current scope:** one strategy for vyākaraṇa, handling sandhi (carry forward from spike) and declension (new).

**Future direction:** after 2-3 strategy implementations exist, extract common patterns into a generic table-driven evaluator that interprets structured claim params as condition/transform rules. This is the path from operation-typed templates (now) to a declarative rule DSL (later). Do not design the DSL prematurely — extract it from working code.

### Rule ordering and conflict resolution

When multiple rules match the same input, conflict resolution is **domain-specific logic within the engine strategy**, not a vidya core feature. For vyākaraṇa, the strategy uses:

- `rule_type` to apply paribhāṣā priority (apavāda beats utsarga, nitya beats anitya)
- `sutra_position` for positional ordering (vipratiṣedhe paraṁ kāryam — later rule wins on conflict)
- Tripādi rules apply in a second pass after all other rules

Other domains may have simpler or no conflict resolution. The engine strategy owns this logic entirely.

### Forward derivation (vidya_derive)

Carries forward from the spike. Given a domain, operation, and input, applies matching rules and returns a result with a step-by-step trace. Each trace step includes the rule applied, the sūtra reference, the input state, and the output state.

Operations are domain-strategy-specific. For vyākaraṇa: `sandhi` (existing), `declension` (new — given stem type + vibhakti + vacana, produce inflected form with trace).

### Reverse analysis (vidya_analyze) — NEW

Given a domain, operation, and surface form, enumerates valid decompositions. Returns a ranked list of candidates, each with:

- The decomposition (e.g., first="tasya", second="iṣṭam")
- The rule that produced the combined form (e.g., guṇa sandhi 6.1.87)
- A specificity score (more specific rule matches rank higher)

Different return shape from `vidya_derive` because the semantics differ: forward produces one deterministic result, reverse produces a search over candidates. Performance characteristics also differ — reverse may need depth limits and pruning.

### Knowledge cluster model

A vidya instance serves a **knowledge cluster** — a set of related domains in one Postgres database that may reference each other. Within a cluster:

- `domain_id` provides logical isolation — queries default to one domain
- Cross-domain queries are opt-in (specify multiple domains or omit the filter)
- Cross-domain relations are supported: the relation belongs to whichever domain defines its `relation_kind`, and src/dst entities can be from any domain in the cluster

Unrelated knowledge (e.g., geology vs. Sanskrit grammar) lives in a separate vidya instance — separate database, separate systemd service, separate MCP endpoint. This is an operational decision, not a schema change.

### MCP tool surface

| Tool | Purpose | Status |
|---|---|---|
| `vidya_health` | Service health check | exists |
| `vidya_domain` | CRUD on domains + ontology (entity kinds, relation kinds, claim templates) | exists |
| `vidya_entity` | CRUD on entities | exists |
| `vidya_claim` | CRUD on claims + assertions | exists |
| `vidya_relation` | CRUD on relations | clarify from existing |
| `vidya_query` | Structured queries (see below) | exists, needs expansion |
| `vidya_load` | Bulk seed from JSON, idempotent, validates against ontology | exists, needs validation |
| `vidya_derive` | Forward derivation with traced result | exists |
| `vidya_analyze` | Reverse analysis — candidate decompositions | new |

#### vidya_query capabilities

The query tool must support agents without requiring them to understand vidya's internals:

- **Entity queries** — by kind, by name pattern, by attribute predicates (e.g., "all vowels where class=guṇa")
- **Claim queries** — by entity, by template type, filtered by tradition and/or pramāṇa
- **Relation traversal** — from an entity, follow relation kind with configurable depth (at least 1-hop, ideally up to N)
- **Provenance queries** — given a claim, return its assertion chain and derivation chain back to source claims
- **Cross-entity predicate queries** — "all entities of kind Z where claim template W has param P = V"

### Seed data and curation

- **Bulk loading** via `vidya_load` from JSON files, carrying forward from the spike. Idempotent (dedup on domain + template + params hash). Must validate claim params against the template's `param_schema` and return actionable errors.
- **LLM-assisted extraction** is the primary scale path. An agent reads source material (via kosha or directly), produces structured JSON, loads it via `vidya_load`, and a human spot-checks. Known extraction sources:
  - Ashtadhyayi simulator's sūtrapāṭha (`aRt_new`) — ~4000 sūtras, machine-readable in WX transliteration, needs WX→IAST conversion
  - Amarakosha CSV (`all_kANdas`) — ~10,000 word entries with 14 relation types, needs WX→IAST conversion
  - Dhātupāṭha — verb root list with meanings and gaṇa assignments
- **Curation workflow varies by domain.** For vyākaraṇa, one canonical tradition — LLM extracts, human spot-checks. For jyotiṣa, practitioners actively curate which claims they endorse — the tradition-scoped assertion model supports this, but the workflow is editorial, not mechanical.

### Deployment

Standard manas service pattern:

- `systemctl --user {start|stop|status} vidya`
- Binary at `~/.cargo/bin/vidya`
- Postgres database (same Postgres instance as chitta, separate database)
- MCP endpoint on a fixed port (e.g., `http://127.0.0.1:4300/mcp`)
- Stdio transport also supported for direct Claude Code integration
- `vidya` with no arguments prints help menu
- `vidya --version` prints version

## Testing Decisions

### What makes a good test

Tests validate external behavior through vidya's MCP interface, not internal implementation details. A test loads seed data, calls a tool, and asserts on the response shape and content.

### Test harness per claim template type

Every claim template type that participates in derivation gets a set of (input, expected_output) pairs stored alongside the seed data. The engine runs these as integration tests against a real Postgres database. For vyākaraṇa:

- **Sandhi tests** — (first, second) → expected result + expected trace steps. Existing: 10 vowel sandhi cases. Expand to consonant and visarga sandhi.
- **Declension tests** — (stem, vibhakti, vacana) → expected inflected form + trace. Use Ruppel's paradigm tables as expected output.
- **Paradigm tests** — full declension table for a stem (24 forms). Tests that multiple rules compose correctly.

### Reference oracle

Use the Zen library (Sanskrit Heritage Platform) as an independent reference implementation. Agreement between vidya's derivations and Zen's output validates correctness. Disagreement pinpoints where to investigate.

### Knowledge layer tests

For non-reasoning domains: load seed data, query entities/claims/relations, verify graph structure. Ontology validation (param schema conformance) tested at the loader level.

## Out of Scope

- **Pāṇini product** — web UI, pedagogical sequencing, curriculum ordering, Ruppel-informed progression. Separate project, separate PRD. Vidya provides the knowledge backend; the product provides the experience.
- **Declarative rule DSL** — the generic condition/transform language for claim templates. Deferred until 2+ engine strategies exist to extract the pattern from. The PRD designs *toward* this (machine-interpretable params, engine trait), but does not specify the DSL.
- **Embedded scripting** (Rhai/Lua/WASM) or dynamic library plugins for engine strategies. Considered and deferred.
- **Embedding/vector search** — vidya is structural. Semantic search is kosha's domain.
- **Automaton/transducer layer** — Zen-style segmenters and recognizers sit above vidya in the Pāṇini product, consuming vidya's knowledge via MCP.
- **Trie/DAG storage** for compact paradigm tables — a Pāṇini product concern, not vidya core.
- **CLI curation tool** beyond `vidya_load` — interactive editing, tab completion, etc.

## Further Notes

### Notes for the Pāṇini product PRD

These came up during vidya design and should be addressed in the Pāṇini PRD:

- **Zen as architectural reference** — Zen's phase-based segmentation (regular grammar over lexical categories), external/internal transition split for sandhi, and coroutine-based lazy parse enumeration are proven designs worth studying for the segmenter layer.
- **Trie + DAG minimization** — Zen's hash-consing approach for morphological paradigm tables maps to Rust's `Arc`-interning. Relevant when the product needs to serve large paradigm tables efficiently.
- **Backward chaining UX** — sandhi splitting needs to show all valid decompositions ranked by specificity. The Pāṇini product decides how to present these; vidya provides the candidates via `vidya_analyze`.
- **Pedagogical sequencing** — Ruppel's ordering (vowel sandhi → consonant sandhi → visarga → declension → conjugation → compounds → participles) determines which vidya rules to encode first, but the sequencing logic lives in the product.
- **Curation differences by domain** — vyākaraṇa accepts one canonical tradition (LLM extracts, human checks). Jyotiṣa requires practitioner editorial judgment on which claims to assert. The product UX may need to reflect this difference.

### Relation to external tools

- **Ashtadhyayi simulator** — source of sūtrapāṭha seed data (~4000 sūtras with coordinates). Rule taxonomy (sandhi, aṅga-vidhi, pratyaya-vidhi, tripādi) informs claim template categories. Architecture (imperative if/else chain) is explicitly *not* a model for vidya.
- **Zen / Sanskrit Heritage Platform** — reference oracle for testing. Architectural ideas (automata, tries, phase segmentation) belong to the Pāṇini product layer, not vidya core.
- **Amarakosha** — the text lives in kosha (document intelligence). The structured knowledge (synsets, 14 relation types, Vaisheshika ontology) lives in vidya as a kosha domain. Validates that vidya's schema handles rich semantic relations without modification.
- **Prolog/Datalog** — conceptual influence, not direct dependency. Datalog's distinction between extensional and intensional predicates maps to vidya's entity/claim vs. derivation split. Semi-naive evaluation is relevant for incremental rule application. Stratification maps to Pāṇinian rule ordering. The eventual declarative rule DSL will likely resemble Horn clauses with domain-specific conflict resolution.
