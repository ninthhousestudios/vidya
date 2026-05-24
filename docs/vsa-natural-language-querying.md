# VSA-Based Natural Language Querying for Vidya

Status: ideas doc, not a plan.
Date: 2025-05-23

## Problem

Vidya's query interface is fully structured: `describe`, `search`, `traverse`,
`provenance`, each requiring exact entity names, predicates, and types. This
works well for MCP tool calls from agents that already know the schema, but
it's unusable from the command line for exploratory questions like:

- "what planets are exalted in fire signs?"
- "tell me about Mars"
- "what does Parashara say about Rahu's dignity?"
- "substances that pacify vata and have ushna veerya"

An LLM could translate these to structured queries, but that defeats vidya's
purpose as an offline, authoritative knowledge store. We want something that
runs locally with zero external dependencies.

## Prior Art: akh-medu

[akh-medu](../../akh-medu) is a neuro-symbolic engine (Rust, GPLv3) that
solves a version of this problem using three techniques worth studying:

### 1. Binary Bipolar VSA (Vector Symbolic Architecture)

10,000-bit binary hypervectors where each component is +1/-1 (stored
bit-packed). Three core operations:

| Operation | Implementation | Purpose |
|-----------|---------------|---------|
| **Bind** | XOR | Associate two concepts (role-filler pairs) |
| **Bundle** | Majority vote | Combine multiple vectors into a superposition |
| **Similarity** | Hamming distance | Find nearest neighbors |

Symbols are deterministically encoded (seeded RNG from symbol ID), then
**re-grounded** after ingestion by bundling each symbol's graph neighborhood:

```
entity_vec = bundle(
    neighbor_1,
    neighbor_2,
    bind(predicate_1, object_1),  // role-filler pair
    bind(predicate_2, object_2),
    ...
)
```

This causes related entities to converge in vector space without any learned
embeddings. "Mars" and "Aries" end up close because they share graph structure
(Mars rules Aries, both are fire-natured, etc.).

Approximate nearest-neighbor search uses HNSW indexing (`hnsw_rs` crate).

Reference: `akh-medu/src/vsa/` — item memory, SIMD-accelerated operations,
HNSW index.

### 2. Role-Filler Recovery via Unbind

Given a subject and predicate, recover the likely object:

```
result_vec = unbind(subject_vec, predicate_vec)  // XOR for binary bipolar
candidates = hnsw_search(result_vec, top_k=5)
```

This is the VSA equivalent of `traverse`: "what is Mars exalted in?" becomes
`unbind(mars_vec, exalted_in_vec)` → HNSW search → returns entities near the
result vector, ideally including Capricorn.

The beauty is that this works even with noisy or incomplete graphs because
the high dimensionality (10,000 bits) provides error tolerance.

Reference: `akh-medu/src/vsa/mod.rs` — `unbind` is just XOR again (XOR is
its own inverse in binary bipolar).

### 3. Analogy via Bind + Unbind

"A is to B as C is to ?" becomes:

```
relation_vec = bind(a_vec, b_vec)
result_vec = unbind(relation_vec, c_vec)
candidates = hnsw_search(result_vec, top_k=5)
```

Example: `bind(king, man)` captures the "royalty" relation;
`unbind(royalty, woman)` → queen.

For vidya's domains this could support queries like "what is to Pitta as
Madhura rasa is to Vata?" (answer: Tikta/Kashaya).

### 4. Four-Tier NLU Pipeline

akh-medu cascades through four parsing tiers:

1. **Rule parser** (<1ms, handles ~70% of input) — hand-rolled recursive
   descent. Priority cascade: questions → commands → declaratives → freeform.
   Outputs `AbsTree` (abstract semantic tree).

2. **Micro-ML NER** (~5ms, feature-gated) — DistilBERT ONNX for entity
   recognition. 130MB model.

3. **Small LLM translator** (~800ms, feature-gated) — Qwen2.5-1.5B with
   GBNF-constrained decoding to guarantee valid output structure. 1.1GB model.

4. **VSA parse ranker** (zero RAM) — accumulates successful parse exemplars,
   uses Jaccard similarity to match new inputs against past successes.
   Self-improving over time.

For vidya, only Tier 1 is interesting. Tiers 2-3 add 1.2GB of model weight,
and Tier 4 is still using Jaccard rather than actual VSA matching.

Reference: `akh-medu/src/nlu/`, `akh-medu/src/grammar/parser.rs`

## What This Could Look Like in Vidya

### Layer 1: VSA Entity Index (vidya-core)

On domain load, encode every entity as a hypervector from its graph
neighborhood and build an HNSW index. This gives:

- **Fuzzy entity resolution**: "mars" matches `jyotish:mangala` even without
  an explicit alias, because the VSA encoding captures structural similarity.
- **Similarity search**: "what's related to Mars?" returns entities that share
  graph structure (Aries, Scorpio, fire element, etc.).
- **Role-filler queries**: "what is Mars exalted in?" via unbind + HNSW.

The encoding happens once per domain load and lives in memory alongside the
oxigraph store. Rough estimate: 10,000 bits × N entities × HNSW overhead.
For vidya's current scale (~1000 triples, ~50 entities per domain), this is
negligible.

### Layer 2: NL Pattern Matcher (CLI front-end)

A simple pattern matcher that maps common question forms to vidya's existing
query modes:

| Pattern | Maps to | Example |
|---------|---------|---------|
| "tell me about X" / "what is X" / "describe X" | `describe` | "tell me about Mars" → `describe mangala` |
| "what Xs have Y" / "find Xs where Y=Z" | `search` | "what grahas have fire element?" → `search graha -f element=fire` |
| "what is X's Y" / "what does X Y" | `traverse` | "what does Mars rule?" → `traverse mangala rules` |
| "what does T say about X" | `provenance` | "what does Parashara say about Rahu?" → `describe rahu --tradition bphs` |
| "what's related to X" / "similar to X" | VSA similarity | HNSW search from entity vector |

This doesn't need to be a full NL parser. A cascade of regex patterns with
VSA-backed entity resolution would cover the common cases. Unrecognized input
falls back to VSA similarity search on the raw terms — better than an error.

### Layer 3: Tradition-Aware VSA (future)

Encode tradition-specific views as separate vector spaces or compartments.
"What does Parashara say Mars is exalted in?" would search only within the
BPHS tradition's vector subspace. This preserves vidya's core value
proposition (tradition-scoped knowledge) in the VSA layer.

## What NOT to Do

- **Don't adopt akh-medu wholesale.** The scope mismatch is too large (agent
  framework, TUI, federation, OODA loop, email/calendar). Vidya would inherit
  massive surface area to solve a query problem.

- **Don't use akh-medu's knowledge graph model.** It doesn't support
  tradition-scoped querying, pramana classification, or RDF-star provenance
  annotations. Confidence values reset to 1.0 on restart (known limitation).

- **Don't add Tiers 2-3 of the NLU pipeline.** 1.2GB of model files for NER
  and LLM fallback defeats the lightweight-offline goal. The rule parser
  (Tier 1) covers the common cases; VSA similarity covers the rest.

## Open Questions

- **Vector dimensionality**: akh-medu uses 10,000 bits. Is that necessary for
  vidya's scale (~50-200 entities per domain)? Lower dimensions (1,000-4,000)
  might suffice and reduce memory/compute. The information-theoretic capacity
  of binary bipolar VSA scales with dimensionality — need to test what
  threshold preserves useful similarity structure at vidya's entity counts.

- **Encoding stability**: When new triples are added via `vidya_assert`, do we
  re-encode affected entities incrementally or rebuild the full index?
  Incremental is cheaper but may cause drift.

- **Cross-domain similarity**: Should VSA indices be per-domain (matching
  vidya's named-graph isolation) or unified? Per-domain is simpler and
  matches the current query model.

- **SIMD**: akh-medu uses AVX2 runtime dispatch for XOR/popcount on x86_64.
  Worth doing from the start or premature optimization at vidya's scale?

## Dependencies (estimated)

- `bitvec` or manual bit-packing for hypervector storage
- `hnsw_rs` + `anndists` for approximate nearest-neighbor (same as akh-medu)
- No model files, no ONNX runtime, no LLM
