# Natural Language Queries

Vidya's CLI commands accept free-text input instead of requiring exact entity
names and structured flags. You still pick the query mode (`describe`, `search`,
`traverse`, `provenance`) — the NL layer resolves your arguments within that
mode.

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

Your input is tokenized, stopwords stripped, and each token matched against
this vocabulary using a cascade:

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
- **Map property values to filters**: fire → element=fire, malefic → nature=malefic,
  movable → quality=movable
- **Correct minor typos**: mangla → mangala (edit distance 1)
- **Report what was resolved**: prints to stderr so you can verify
- **Report unknown tokens**: if part of your input wasn't recognized, it tells you
- **Stay backwards compatible**: exact names and structured flags work as before

## What it cannot do

- **No English synonyms.** "planets" does not resolve to Graha. You must use
  domain vocabulary: graha, rashi, bhava, etc. Same for "signs" (use rashi),
  "houses" (use bhava).

- **No intent detection.** You must pick the mode yourself. "what is Mars
  exalted in?" doesn't auto-detect that you want `traverse` — you must run
  `vidya traverse`.

- **No multi-word entity matching.** Entities with spaces in their labels
  (like "1st House") won't resolve from free text because tokenization splits
  on whitespace.

- **No type inference.** `vidya search -d jyotish "fire"` fails because
  there's no type token. You must include the type: `"fire graha"`.

- **Edit distance is noisy for short words.** A 2-letter typo in a 4-letter
  word matches many candidates. Works best for longer terms (6+ characters).

- **No cross-domain resolution.** Resolution operates within a single domain.

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
