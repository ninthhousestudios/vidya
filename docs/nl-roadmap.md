# Natural Language Resolution Roadmap

Status: active plan.
Date: 2025-05-25.
Source: `docs/nl-extension.md` (design doc) + yojana task graph.

## Current state

Vidya resolves free-text tokens within an explicitly chosen query mode
(describe, search, traverse, provenance). The resolution cascade is:
exact match > substring > edit-distance > HRR/VSA similarity. This handles
"mars" -> mangala, "fire graha" -> search Graha element=fire, etc.

What it cannot do yet:
- Infer the query mode from a natural-language question
- Resolve multi-word phrases ("natural friend", "exalted in")
- Recognize English synonyms ("planets" -> Graha)
- Infer search type when only a property value is given ("fire" alone)
- Expose VSA similarity/unbind as query operations
- Handle tradition-scoping phrases ("according to Parashara")

## Execution order

The dependency graph flows top-to-bottom. Each wave can run in parallel
within itself.

```
Wave 1 (prerequisite cleanup)
  vidya/34  Filter RDF meta-types from SchemaVocab         [AFK, ready]

Wave 2 (resolver improvements — run in parallel)
  vidya/33  Multi-word entity matching in NL tokenizer      [AFK, blocked on 34]
  vidya/32  English synonym table for NL resolution         [AFK, blocked on 34]
  vidya/31  Infer search type when no type token present    [AFK, ready]
  vidya/30  VSA similar + unbind CLI/MCP commands           [AFK, ready, independent]

Wave 3 (sentence-level understanding)
  vidya/36  Intent pattern matcher for mode-less queries    [AFK, blocked on 33+32]

Wave 4 (refinements — run in parallel)
  vidya/37  Deterministic parse ranking                     [AFK, blocked on 36]
  vidya/38  Tradition-aware NL resolution                   [AFK, blocked on 36]
```

Path 9 from the design doc (learning from confirmed parses) is deferred
until real usage shows repeated phrasing patterns.

## Wave 1: Clean vocabulary (vidya/34)

Remove rdf:Property, rdfs:Class, and similar RDF infrastructure types and
meta-predicates from SchemaVocab. These leak into vocab output and NL
matching as noise. Domain-meaningful vidya: types (Tradition, Source) stay.

Isolated change in `vidya-core/src/resolve/vocab.rs`. All downstream NL
work benefits from a clean vocabulary.

## Wave 2: Resolver improvements

### Multi-word phrases (vidya/33)

After tokenization, run a greedy longest-first n-gram pass against
SchemaVocab before single-token resolution. Bigrams cover the common cases:
"natural friend", "exalted in", "1st house".

Change in `vidya-core/src/resolve/matcher.rs`.

### English synonyms (vidya/32)

**Decision:** use separate per-domain lexicon files (e.g.
`seeds/jyotish-synonyms.toml`), not rdfs:label in TTL seeds or hardcoded
tables. Synonyms are editorial/UX concerns, not ontological facts.

Synonym categories: type synonyms (planets->Graha), predicate paraphrases
(rules->rules, friendly to->naturalFriend), property value paraphrases,
entity aliases, tradition/source aliases.

The resolver should tag each match with its source (canonical vs. synonym)
in the resolution report.

### Type inference for search (vidya/31)

When search has property-value matches but no type token, infer the type by
checking which types have entities with that property value. If ambiguous,
return a disambiguation result listing options.

Change in `vidya-core/src/resolve/assemble.rs`, may need a reverse index
in SchemaVocab.

### VSA similar + unbind (vidya/30)

Expose two new query operations from the existing EntityIndex:
- `similar(subject, top_k)` — nearest-neighbor search over entity vectors
- `unbind(subject, predicate, top_k)` — role-filler recovery

CLI commands + MCP tools. Independent of the other resolver improvements.
Results must be clearly marked as approximate/structural similarity, not
asserted facts.

## Wave 3: Intent detection (vidya/36)

A deterministic pattern cascade that maps common question forms to query
modes. Lives in a new `resolve::intent` module above the existing matcher.

Core patterns:
- "tell me about X" -> describe
- "what is X's Y" / "what does X Y" -> traverse
- "what Xs are Y" / "find Xs where Y is Z" -> search
- "what is related to X" -> similar
- "what does T say about X" -> tradition-scoped query

The intent layer decides query shape; the existing matcher fills slots.
This is the payoff point where waves 1-2 compound.

## Wave 4: Refinements

### Parse ranking (vidya/37)

When intent detection + resolver produce multiple candidate parses, rank
them by: token coverage, match tier (exact > alias > synonym > edit-distance
> VSA), unknown token count, ontology validity, whether an exact graph
result exists. Expose scored candidate lists for debuggability.

### Tradition-aware resolution (vidya/38)

Handle "according to BPHS", "what does Parashara say" as tradition/source
filter extraction during intent parsing. Resolve tradition/source/pramana
names as first-class vocabulary entries.

## Design constraints

From `docs/nl-extension.md`:

- No LLM, no model files, no remote API
- Every NL path reports what it resolved, ignored, and confidence source
- No prose answer generation — output is always structured queries
- No silent tradition blending
- No hardcoded domain semantics that belong in domain data
- Approximate (VSA) results are always distinguished from graph facts

## Testing approach

Tests operate through public query APIs. Each wave adds test cases for:
- The specific resolution improvement (phrase matching, synonym lookup, etc.)
- Regression against existing resolution behavior
- Both the structured query result and the resolution report
