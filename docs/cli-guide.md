# vidya CLI guide

A walk through the CLI from basic to advanced. All examples use real data
from the jyotish and ayurveda seeds.

## Setup

```
vidya load jyotish seeds/jyotish.ttl
vidya load ayurveda seeds/ayurveda.ttl
vidya domains
```

If the systemd service is running, stop it first (`systemctl --user stop
vidya`) — loading requires a write lock. Querying works fine while the
service runs.

Set a default domain to skip `-d` on every command:

```
export VIDYA_DOMAIN=jyotish
```

## Basics: what's in a domain?

### List all entities of a type

```
vidya search -d jyotish Graha
vidya search -d jyotish Rashi
vidya search -d jyotish Bhava
vidya search -d ayurveda Dravya
```

### Describe an entity

```
vidya describe -d jyotish surya
```

Output shows the label, types, flat properties, and annotated triples
(relationships with provenance). Surya's output includes:

- Properties: element (fire), nature (malefic), gender (masculine),
  aliases (Sun, Arka, Ravi), karakas (soul, father, authority, etc.)
- Relationships: exalted in Mesha, debilitated in Tula, rules Simha,
  friends (Chandra, Guru, Mangala), enemies (Shani, Shukra)
- Each relationship carries provenance: tradition, source, pramana,
  confidence

### Use aliases and Western names

You don't need to know the Sanskrit name. Alias resolution searches
`rdfs:label`, `alias`, and `western` properties case-insensitively:

```
vidya describe -d jyotish Sun          # → Sūrya
vidya describe -d jyotish Jupiter      # → Guru
vidya describe -d jyotish Cancer       # → Karka
vidya describe -d jyotish Aries        # → Meṣa
vidya describe -d jyotish "North Node" # → Rāhu
```

## Filtering: narrowing results

### By attribute

`-f key=value` filters search results. Multiple filters combine as AND:

```
# Fire-element grahas
vidya search -d jyotish Graha -f element=fire
→ Maṅgala, Sūrya

# Benefic grahas
vidya search -d jyotish Graha -f nature=benefic
→ Chandra, Guru, Śukra

# Water-sign rashis
vidya search -d jyotish Rashi -f element=water
→ Karka, Mīna, Vṛścika

# Movable rashis
vidya search -d jyotish Rashi -f quality=movable
→ Karka, Meṣa, Tulā, Makara

# Bitter (tikta) dravyas
vidya search -d ayurveda Dravya -f hasRasa=tikta
→ amalaki, ashwagandha, brahmi, guduchi, guggulu, haridra, ...

# Pungent AND hot substances
vidya search -d ayurveda Dravya -f hasRasa=katu -f hasVeerya=ushna
→ ashwagandha, eranda, guggulu, haridra, maricha, pippali, shunthi, tulasi
```

### By tradition

`--tradition` scopes any query to assertions from a specific tradition.
This is the key feature — when traditions disagree, you can ask each one
separately:

```
# What does the Atreya tradition (Charaka) say about pippali?
vidya describe -d ayurveda pippali --tradition tradition-atreya
→ hasVeerya → sheeta (cold), confidence 0.85

# What does the Dhanvantari tradition (Sushruta) say?
vidya describe -d ayurveda pippali --tradition tradition-dhanvantari
→ hasVeerya → ushna (hot), confidence 0.9
```

Pippali's veerya is contested — Charaka says sheeta, Sushruta says
ushna. Without the tradition filter, both appear.

### By pramana

`--pramana` filters by epistemological basis:

```
vidya describe -d jyotish surya --pramana vidya:shabda
```

The six pramanas are: shabda (testimony), pratyaksha (perception),
anumana (inference), upamana (analogy), arthapatti (presumption),
anupalabdhi (non-apprehension). Most seed data uses shabda since
it comes from authoritative texts.

## Traversal: walking relationships

`traverse` follows a predicate from an entity through the graph.

### Direct relationships (depth 1)

```
# Who are Surya's natural friends?
vidya traverse -d jyotish surya naturalFriend
→ Chandra, Guru, Maṅgala

# Who does Shani consider enemies?
vidya traverse -d jyotish shani naturalEnemy
→ Sūrya, Maṅgala, Chandra

# Which rashi does Surya rule?
vidya traverse -d jyotish surya rules
→ Siṃha

# What does Guru aspect?
vidya traverse -d jyotish guru aspectsHouse
→ 5, 7, 9
```

### Multi-hop (depth 2+)

```
# Friends of friends of Surya
vidya traverse -d jyotish surya naturalFriend --depth 2
→ depth 1: Chandra, Guru, Maṅgala
  depth 2: Budha (friend of a friend, but not Surya's direct friend)

# Guru's friend network at depth 2
vidya traverse -d jyotish guru naturalFriend --depth 2
→ depth 1: Chandra, Maṅgala, Sūrya
  depth 2: Budha
```

This is useful for finding indirect relationships the graph encodes
but that aren't obvious from a single entity's profile.

## Provenance: who says what, and why

`provenance` returns the epistemological metadata for a specific triple.

### Confident assertions

```
vidya provenance -d jyotish surya exaltedIn mesha
→ tradition: tradition-bphs
  source:    source-bphs
  pramana:   vidya:shabda
  confidence: 1
```

Confidence 1 — no disagreement. BPHS (Bṛhat Parāśara Horā Śāstra) says
Surya is exalted in Mesha, and nobody disputes it.

### Contested assertions

```
# Pippali veerya according to Sushruta/Bhavaprakasha
vidya provenance -d ayurveda pippali hasVeerya ushna
→ tradition: tradition-dhanvantari, source: source-sushruta, confidence: 0.9
  tradition: tradition-dhanvantari, source: source-bhavaprakasha, confidence: 0.9

# Pippali veerya according to Charaka
vidya provenance -d ayurveda pippali hasVeerya sheeta
→ tradition: tradition-atreya, source: source-charaka, confidence: 0.85
```

Two traditions, two answers. This is exactly what vidya is for —
surfacing disagreement rather than silently picking one.

### Low-confidence assertions

```
vidya describe -d jyotish rahu
```

Rahu's dignities carry confidence 0.7 — the shadow planets' exaltation
and debilitation signs are not universally agreed upon. The data encodes
this uncertainty rather than presenting one answer as fact.

### Alias resolution in provenance

Both subject and object resolve through aliases:

```
vidya provenance -d jyotish Sun exaltedIn Aries
→ tradition: tradition-bphs, source: source-bphs, ...
```

"Sun" → surya, "Aries" → mesha.

## Cross-domain exploration

Vidya's domains are independent named graphs. You can compare how
different knowledge systems categorize and relate concepts:

```
# Jyotish: what element is Surya?
vidya describe -d jyotish surya
→ element: fire

# Ayurveda: what are the hot (ushna veerya) substances?
vidya search -d ayurveda Dravya -f hasVeerya=ushna

# Jyotish: which houses does Guru aspect?
vidya traverse -d jyotish guru aspectsHouse
→ 5, 7, 9

# Ayurveda: what does ashwagandha do?
vidya describe -d ayurveda ashwagandha
→ hasKarma: rasayana, vajikarana
  hasRasa: kashaya, katu, tikta
  hasVeerya: ushna
  pacifiesDosha: vata, kapha
  aggravatesDosha: pitta
```

## JSON output

Add `--json` for machine-readable output. Useful for piping to `jq`:

```
vidya describe -d jyotish surya --json | jq '.properties[] | select(.predicate == "karaka") | .value'
→ "authority", "bones", "ego", "father", ...

vidya search -d jyotish Graha -f element=fire --json | jq '.entities[].name'
→ "mangala", "surya"
```

## Quick reference

| Task | Command |
|------|---------|
| Load a domain | `vidya load <domain> <file.ttl>` |
| List domains | `vidya domains` |
| Describe entity | `vidya describe -d <domain> <name>` |
| Search by type | `vidya search -d <domain> <Type>` |
| Filter by attribute | `vidya search -d <domain> <Type> -f key=value` |
| Walk relationships | `vidya traverse -d <domain> <entity> <predicate>` |
| Multi-hop walk | `vidya traverse -d <domain> <entity> <predicate> --depth N` |
| Triple provenance | `vidya provenance -d <domain> <subj> <pred> <obj>` |
| Scope to tradition | add `--tradition <name>` |
| Scope to pramana | add `--pramana vidya:<type>` |
| Machine output | add `--json` |
| Default domain | `export VIDYA_DOMAIN=<domain>` |
