# Non-LLM Natural Language Extensions

Status: design note.
Date: 2026-05-25

## Purpose

Vidya currently supports natural-language-like argument resolution, not full
natural-language querying. The caller still chooses a query mode (`describe`,
`search`, `traverse`, or `provenance`), and the resolver maps free-text tokens
inside that mode to known entities, types, predicates, and property values.

The current cascade is:

1. Exact name, alias, label, type, predicate, or property-value match
2. Substring match
3. Edit-distance match
4. HRR/VSA similarity fallback

This makes commands like `describe mars`, `search "fire graha"`, and
`traverse mars rules` forgiving. It does not yet infer that "what is Mars
exalted in?" should become a `traverse` query.

This document describes ways to extend Vidya's natural-language ability without
using an LLM. The goal is a local, deterministic, inspectable query layer that
preserves Vidya's structured data model and provenance guarantees.

## Design Boundary

Non-LLM querying should translate input into Vidya's existing structured query
forms. It should not become an open-ended answer generator.

Good outputs:

- `Describe { subject }`
- `Search { type, filters }`
- `Traverse { subject, predicate }`
- `Provenance { subject, predicate, object }`
- `Similar { subject, top_k }`
- `ApproximateTraverse { subject, predicate, top_k }`

Bad outputs:

- Prose answers invented from partial matches
- Silent blending of traditions or sources
- Uninspectable ranking decisions
- Domain facts encoded only in Rust heuristics when they belong in domain data

Every natural-language path should report what it resolved, what it ignored,
and what confidence source produced each match.

## Path 1: Intent Pattern Matcher

The most valuable missing layer is intent detection. A small cascade of
patterns can map common question forms to Vidya's existing query modes before
token resolution runs.

Example patterns:

| Input shape | Structured query |
| --- | --- |
| `tell me about X` | `describe X` |
| `what is X` | `describe X`, unless a predicate phrase is present |
| `describe X` | `describe X` |
| `what is X's Y` | `traverse X Y` |
| `what does X Y` | `traverse X Y` |
| `what does X rule` | `traverse X rules` |
| `what Xs are Y` | `search X` with property filter `Y` |
| `find Xs where Y is Z` | `search X -f Y=Z` |
| `what does T say about X` | query scoped by `tradition=T` |
| `what is related to X` | `similar X` |
| `similar to X` | `similar X` |

This does not need a full parser. A deterministic pattern cascade is enough if
it produces a small set of candidate structured queries and lets the resolver
score them.

Implementation shape:

1. Normalize input: lowercase, strip punctuation except meaningful separators.
2. Match phrase patterns in priority order.
3. Extract slots such as `subject`, `predicate`, `type`, `filter`, and
   `tradition`.
4. Run existing vocabulary/VSA resolution on each slot.
5. Assemble one or more structured query candidates.
6. Rank candidates by validity and confidence.

Pattern matching should live above `resolve::matcher`. The existing matcher is
good at classifying tokens; intent detection decides what query shape those
tokens should fill.

## Path 2: Domain Synonyms and Paraphrases

The current vocabulary is derived from labels, aliases, local names, predicates,
and property values. It intentionally does not know English paraphrases like
"planets" for `Graha` or "signs" for `Rashi` unless those strings exist in the
domain data.

Vidya can extend this without an LLM by making synonyms first-class domain
metadata.

Examples:

| Phrase | Resolves to |
| --- | --- |
| `planet`, `planets` | `Graha` |
| `sign`, `signs`, `zodiac sign` | `Rashi` |
| `house`, `houses` | `Bhava` |
| `rules`, `lord of`, `ruler of` | `rules` predicate |
| `friend`, `friendly to` | `naturalFriend` predicate |
| `enemy`, `hostile to` | `naturalEnemy` predicate |
| `hot`, `fiery` | `element=fire` or domain-specific equivalent |

These mappings should be loaded from RDF or a small domain-side lexicon, not
hardcoded into the resolver. The same English word can mean different things in
different domains. For example, `karma` means different things in jyotish and
ayurveda, and the synonym system must preserve domain isolation.

Useful synonym categories:

- Type synonyms
- Entity aliases
- Predicate paraphrases
- Property value paraphrases
- Tradition/source aliases
- Query intent phrases

The resolver should keep the source of each synonym in the resolution report so
the caller can distinguish canonical labels from looser paraphrases.

## Path 3: Multi-Word Phrase Resolution

Current token resolution is mostly word-by-word. That misses useful phrases:

- `1st House`
- `natural friend`
- `fire sign`
- `lord of`
- `exalted in`
- `debilitated in`
- `hot potency`

A phrase resolver should run before single-token resolution.

Recommended approach:

1. Generate n-grams from the normalized input, longest first.
2. Resolve each n-gram against entity aliases, predicate paraphrases, type
   names, and value phrases.
3. Mark consumed spans.
4. Fall back to single-token resolution for unconsumed tokens.

Longest-first matching avoids splitting `natural friend` into two unrelated
tokens. Ambiguous phrases should produce multiple candidates rather than
choosing silently.

This layer pairs well with the synonym lexicon. Multi-word synonyms are common
for predicates and domain concepts.

## Path 4: Type Inference for Search

Today, search requires a type. For example, `fire` cannot become a complete
search because Vidya does not know whether the user wants fire grahas, fire
rashis, fire herbs, or something else.

Type inference can produce candidate searches:

- `fire` -> `Graha where element=fire`
- `fire` -> `Rashi where element=fire`
- `benefic` -> `Graha where nature=benefic`
- `movable` -> `Rashi where quality=movable`

The resolver can derive these candidates by asking: which entity types have
triples using the matched predicate/value pair?

Ranking signals:

- Number of matching entities
- Domain default type preferences
- Whether the input includes plural type synonyms such as `planets` or `signs`
- Prior successful parse history
- User-selected query mode, if present

If several types are plausible, Vidya should return a disambiguation result or
a grouped result rather than guessing. For command-line use, grouped results are
often better than a hard error.

## Path 5: VSA Similarity Query

The HRR/VSA index already encodes each entity from its graph neighborhood:

```text
entity_vec = bundle(
  bind(predicate_1, object_1),
  bind(predicate_2, object_2),
  ...
)
```

This gives a natural non-LLM query type:

```text
similar(subject, top_k)
```

Example inputs:

- `what is related to Mars`
- `similar to Mangala`
- `things like Pitta`
- `entities near Rahu`

This is where VSA is strongest. It does not need to understand English deeply;
it only needs to resolve the subject and run nearest-neighbor search over
entity vectors.

At Vidya's current scale, brute-force search is acceptable. An approximate
nearest-neighbor index can be added later if domains grow enough to justify it.

The result should expose scores and avoid overstating meaning. VSA similarity
means "structurally similar in this graph," not "same," "equivalent," or
"semantically entailed."

## Path 6: VSA Role-Filler Recovery

The HRR index also supports role-filler recovery:

```text
result_vec = unbind(subject_vec, predicate_vec)
candidates = nearest_symbols(result_vec)
```

This can back an approximate traverse query:

```text
approximate_traverse(subject, predicate, top_k)
```

Example inputs:

- `what is Mars exalted in`
- `what does Mangala rule`
- `who is Saturn friendly to`
- `what pacifies Vata`

The exact SPARQL traverse path should remain primary when the subject and
predicate are known. VSA unbind is useful as:

- A fallback when exact traversal returns nothing
- A ranked suggestion mechanism
- A robustness layer over incomplete or noisy data
- A way to support exploratory "likely relation" questions

The UI/API should distinguish exact graph facts from VSA-derived candidates.
Approximate candidates should not be presented as asserted facts unless a
matching triple and provenance are found.

## Path 7: Deterministic Parse Ranking

Once patterns, phrases, synonyms, and type inference exist, one input may
produce several possible parses. Vidya can rank them without an LLM.

Candidate scoring signals:

- Token/span coverage
- Exact match beats alias, alias beats synonym, synonym beats edit distance,
  edit distance beats VSA fallback
- Fewer unknown tokens
- Validity against ontology/query shape
- Exact graph result exists
- VSA similarity score
- Tradition/source filter recognized
- Prior successful parse examples

The output should keep the candidate list available for inspection. A simple
score breakdown makes resolver behavior debuggable:

```text
candidate: Traverse(mangala, exaltedIn)
score: 0.91
signals:
  pattern: "what is X Y" = 0.20
  subject exact alias "mars" = 0.25
  predicate phrase "exalted in" = 0.25
  exact graph result exists = 0.15
  no unknown tokens = 0.06
```

This keeps the system understandable and gives tests stable behavior to assert.

## Path 8: Tradition-Aware Resolution

Vidya's core value is not just fact lookup; it is tradition- and
provenance-aware fact lookup. Natural-language extensions must preserve that.

Useful patterns:

- `what does Parashara say about Rahu`
- `according to BPHS, where is Mars exalted`
- `in western astrology, what rules Scorpio`
- `show claims from shabda pramana about X`

Implementation options:

1. Resolve tradition/source/pramana names as first-class vocabulary entries.
2. Add query-scoped filters during intent parsing.
3. Keep the VSA index per domain initially.
4. Later, build tradition-specific VSA compartments or indexes if approximate
   similarity needs to respect tradition boundaries.

Exact SPARQL provenance filters should remain the authority. VSA should not
blend assertions across traditions unless the user explicitly asks for an
unscoped comparison.

## Path 9: Learning From Confirmed Parses Without ML

Vidya can improve over time without models by storing successful parse
exemplars.

Example:

```text
input: "what signs does mars rule"
parse: Traverse(mangala, rules)
domain: jyotish
```

Future similar inputs can be matched by:

- Normalized token overlap
- Phrase overlap
- Resolved slot similarity
- VSA similarity between resolved entities or predicates

This is not statistical training. It is a cache of inspected examples that can
be replayed, audited, deleted, and tested. It is especially useful for local
phrasing habits and domain-specific idioms.

## Recommended Implementation Sequence

1. Add a new intent layer that maps common question forms to existing query
   modes.
2. Add phrase resolution with longest-first n-gram matching.
3. Add domain-owned synonym/paraphrase vocabulary.
4. Add type inference for search filters.
5. Expose VSA `similar` as a first-class query.
6. Expose VSA `unbind_query` as approximate traverse, clearly marked as
   approximate.
7. Add deterministic parse ranking and candidate reports.
8. Add tradition/source/pramana phrase handling.
9. Add confirmed-parse exemplars if real usage shows repeated phrasing.

This order improves common command-line queries early while keeping each step
testable and reversible.

## Testing Strategy

Tests should operate through public query APIs where possible.

Core cases:

- Intent detection:
  - `tell me about Mars` -> describe `mangala`
  - `what does Mars rule` -> traverse `mangala rules`
  - `what fire grahas exist` -> search `Graha element=fire`
- Phrase resolution:
  - `natural friend` resolves as one predicate phrase
  - `exalted in` resolves as one predicate phrase
  - `1st house` resolves as one entity or value phrase where present
- Synonyms:
  - `planets` -> `Graha`
  - `signs` -> `Rashi`
  - domain-specific collisions stay isolated
- Type inference:
  - `fire` yields grouped candidates instead of a silent guess
  - `fire signs` chooses `Rashi element=fire`
- VSA:
  - `similar to Mars` returns structurally close entities with scores
  - approximate traverse results are marked approximate
- Provenance:
  - `according to BPHS` applies a tradition/source filter
  - unscoped and scoped results remain distinct

Regression tests should assert both the structured query and the resolution
report. The report is part of the safety contract.

## Non-Goals

- No general-purpose English parser.
- No model files, ONNX runtime, remote API, or LLM fallback.
- No prose answer generation in the resolver.
- No hidden tradition blending.
- No hardcoded domain semantics that should live in domain data.

The target is a practical, inspectable layer that handles common natural
question forms while keeping Vidya's structured graph and provenance model in
control.
