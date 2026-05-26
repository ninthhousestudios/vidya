# Natural Language Queries

Vidya supports two levels of natural language:

1. **`vidya ask`** — fully freeform. Auto-detects the query mode from the
   question shape, resolves entity/type/predicate names, and applies
   tradition/source/pramana scoping from the question text.

2. **NL fallback on structured commands** — `describe`, `search`, `traverse`,
   `provenance` accept natural language arguments. You pick the mode; vidya
   resolves your arguments within it.

Structured input still works identically. NL resolution is a fallback that
activates only when the structured path fails.

## How it works

On NL fallback, vidya builds a vocabulary index from the loaded domain:

- **Entity names** — rdfs:label, aliases, western names (e.g. mangala has
  labels "Mangala", aliases "Mars", "Kuja", "Bhumi-putra")
- **Type names** — class local names and labels (e.g. "Graha", "graha",
  "Rashi", "rashi")
- **Predicate names** — relation local names including those inside RDF-star
  quoted triples (e.g. "rules", "exaltedIn", "naturalFriend")
- **Property values** — literal values keyed by predicate (e.g. "fire" under
  element, "malefic" under nature)
- **English synonyms** — from a per-domain TOML file (e.g.
  `seeds/jyotish-synonyms.toml`) that maps "planet" → Graha, "sign" → Rashi,
  "exalted" → exaltedIn, etc.
- **Tradition/source/pramana names** — instances of vidya:Tradition,
  vidya:Source, and vidya:Pramana, by local name, label, and stripped prefix
  (e.g. "bphs" matches `tradition-bphs`)

Your input is tokenized (multi-word entities are matched as phrases before
falling back to individual tokens), stopwords stripped, and each token matched
against this vocabulary using a cascade:

1. **Exact match** (case-insensitive) on any name/alias/label
2. **Substring match** for tokens >= 3 characters
3. **Edit distance** (max 2 edits) for typo correction
4. **HRR VSA similarity** as a final fuzzy fallback

Matched tokens are classified (entity, type, predicate, property value) and
assembled into the structured query the mode expects. When NL resolution is
used, vidya prints what it resolved to on stderr so you can verify.

## Examples by mode

### Describe

Needs: one entity.

```sh
# Western name
vidya describe -d jyotish mars
#=> Mangala (types: Graha) ...

# Sanskrit alias
vidya describe -d jyotish soma
#=> Chandra (types: Graha) ...

# Another alias
vidya describe -d jyotish ravi
#=> Surya (types: Graha) ...

# Hyphenated alias
vidya describe -d jyotish bhumi-putra
#   resolved: subject: mangala
#=> Mangala (types: Graha) ...

# Direct local name (always works, no NL needed)
vidya describe -d jyotish budha
#=> Budha (types: Graha) ...

# Typo correction (edit distance 1)
vidya describe -d jyotish mangla
#   resolved: subject: mangala
#=> Mangala (types: Graha) ...
```

### Search

Needs: a type. Optionally: property values as filters.

```sh
# Type + element filter
vidya search -d jyotish "fire graha"
#   resolved: type: Graha, filter: element=fire
#=> mangala  Mangala
#=> surya    Surya

# Type + nature filter
vidya search -d jyotish "benefic graha"
#   resolved: type: Graha, filter: nature=benefic
#=> chandra  Chandra
#=> guru     Guru
#=> shukra   Shukra

vidya search -d jyotish "malefic graha"
#   resolved: type: Graha, filter: nature=malefic
#=> ketu     Ketu
#=> mangala  Mangala
#=> rahu     Rahu
#=> surya    Surya
#=> shani    Shani

# Type + gender filter
vidya search -d jyotish "masculine graha"
#   resolved: type: Graha, filter: gender=masculine
#=> guru     Guru
#=> mangala  Mangala
#=> surya    Surya

vidya search -d jyotish "feminine graha"
#   resolved: type: Graha, filter: gender=feminine
#=> chandra  Chandra
#=> rahu     Rahu
#=> shukra   Shukra

# Rashi queries
vidya search -d jyotish "water rashi"
#   resolved: type: Rashi, filter: element=water
#=> karka      Karka
#=> mina       Mina
#=> vrischika  Vrischika

vidya search -d jyotish "movable rashi"
#   resolved: type: Rashi, filter: quality=movable
#=> karka   Karka
#=> makara  Makara
#=> mesha   Mesha
#=> tula    Tula

vidya search -d jyotish "fixed rashi"
#   resolved: type: Rashi, filter: quality=fixed
#=> kumbha     Kumbha
#=> simha      Simha
#=> vrischika  Vrischika
#=> vrishabha  Vrishabha

vidya search -d jyotish "dual rashi"
#   resolved: type: Rashi, filter: quality=dual
#=> dhanus   Dhanus
#=> kanya    Kanya
#=> mithuna  Mithuna
#=> mina     Mina

# Structured form still works
vidya search -d jyotish Graha -f element=fire
#=> mangala  Mangala
#=> surya    Surya
```

### Traverse

Needs: one entity + one predicate.

```sh
# Western name + predicate
vidya traverse -d jyotish mars rules
#=> Mesha, Vrischika

vidya traverse -d jyotish mars exaltedIn
#=> Makara

vidya traverse -d jyotish sun naturalFriend
#=> Mangala, Chandra, Guru

vidya traverse -d jyotish moon debilitatedIn
#=> Vrischika

vidya traverse -d jyotish jupiter rules
#=> Mina, Dhanus

vidya traverse -d jyotish saturn naturalEnemy
#=> Surya, Mangala, Chandra

vidya traverse -d jyotish mars naturalFriend
#=> Guru, Chandra, Surya
```

### Provenance

Needs: subject + predicate + object.

```sh
vidya provenance -d jyotish mangala rules mesha
#=> tradition: tradition-bphs
#=> source:    source-bphs
#=> pramana:   vidya:shabda
#=> confidence: 1
```

## What it can do

- **Resolve western names**: Mars → mangala, Sun → surya, Jupiter → guru
- **Resolve Sanskrit aliases**: Soma → chandra, Ravi → surya, Kuja → mangala
- **Resolve hyphenated aliases**: Bhumi-putra → mangala
- **Match types by label**: graha, rashi, bhava, nakshatra (case-insensitive)
- **English synonyms**: "planet" → Graha, "sign" → Rashi, "house" → Bhava,
  "constellation" → Nakshatra (via per-domain synonym table)
- **Map property values to filters**: fire → element=fire, malefic → nature=malefic,
  movable → quality=movable
- **Infer type from value**: `"fire"` alone resolves to Graha search with
  element=fire, because the value→type reverse index knows which types carry
  that property
- **Multi-word entity matching**: "1st House" matches as a phrase before
  individual tokens are considered
- **Intent detection** (`vidya ask`): auto-detects describe, search, traverse,
  similar, tradition-scoped, and pramana-scoped modes from question shape
- **Deterministic ranking**: when input is ambiguous, picks the best
  interpretation using shape-validity and pattern-specificity scoring, and
  shows ranked alternatives
- **Correct minor typos**: mangla → mangala (edit distance 1)
- **Report what was resolved**: prints to stderr so you can verify
- **Report unknown tokens**: if part of your input wasn't recognized, it tells you
- **Stay backwards compatible**: exact names and structured flags work as before

## Current limitations

- **Edit distance is noisy for short words.** A 2-letter typo in a 4-letter
  word matches many candidates. Works best for longer terms (6+ characters).

- **No cross-domain resolution.** Resolution operates within a single domain.

- **Synonym table is per-domain.** Each domain needs its own TOML file
  (e.g. `seeds/jyotish-synonyms.toml`). There's no shared English vocabulary.

## Resolution indicator

When NL resolution activates, vidya prints to stderr:

```
  resolved: type: Graha, filter: element=fire
```

If some tokens weren't recognized:

```
  resolved: type: Graha, filter: element=fire
  unrecognized: xyzzy
```

If nothing could be resolved, you get an error:

```
Error: could not resolve any tokens from input
```

## How it relates to structured queries

NL resolution is a fallback, not a replacement. The resolution order is:

1. Try the structured query path (exact IRI resolution, alias lookup)
2. If that returns NotFound, try NL resolution on the input text
3. If NL resolution succeeds, run the structured query with resolved parameters

This means `vidya describe -d jyotish mangala` never touches the NL layer —
it resolves directly via the existing alias lookup. Only inputs that fail
structured resolution (like `bhumi-putra`, which the old alias lookup didn't
handle) trigger NL.
