# Declension engine design

## Context

vidya/9 calls for a declension operation in the vyakarana engine strategy. During design grilling we identified that declension requires four new rule layers beyond the existing sandhi rules, and that the pipeline architecture should anticipate verb conjugation (which adds vikarana morphemes and multiple junction points).

This doc captures the design decisions for the declension spike, targeting the a-stem masculine paradigm (deva, 24 forms).

## Pipeline architecture

### Five-layer derivation pipeline

Each layer corresponds to a category of Paninian rules and maps to a claim template in vidya:

| Layer | Claim template | Astadhayi section | What it does |
|---|---|---|---|
| 1. Suffix selection | `sup_suffix` | 4.1.2 | Look up the raw pratyaya for (stem_class, vibhakti, vacana) |
| 2. Pratyaya modification | `pratyaya_rule` | scattered | Modify the suffix based on stem class (e.g., sas -> an for a-stem acc pl) |
| 3. Anga modification | `anga_rule` | 6-7 | Modify the stem before the suffix (guna, vrddhi, lengthening) |
| 4. Junction sandhi | `sandhi_rule` | 6.1 | Rules at the stem-suffix boundary (already exists) |
| 5. Tripadi | `tripadi_rule` | 8.2-8.4 | Late-pass: visarga (s->h), retroflexion (s->s), etc. |

The engine runs these in order, accumulating trace steps. Each step cites the sutra that triggered it.

### One-pass for the spike, iterative later

For declension (nominal), there is one morpheme attachment (stem + suffix), so the pipeline runs once. Verb conjugation (tinanta) will need multiple passes (root + vikarana + suffix), creating two junction points. The iterative generalization is deferred — the claim templates and rule data are the same either way, only the orchestration code changes (~50-100 lines to refactor).

### Anubandha metadata

The `sup_suffix` claim stores both the raw pratyaya (with markers) and the processed suffix (markers stripped). Markers are carried as metadata so that downstream layers (especially anga rules) can condition on them. For example, the "n" in "ne" (dative sg) marks the suffix as nit, which triggers specific anga rules.

Example `sup_suffix` params:
```json
{
  "stem_class": "a-stem-m",
  "vibhakti": "caturthi",
  "vacana": "ekavacana",
  "pratyaya": "ne",
  "suffix": "e",
  "markers": ["n"],
  "sutra": "4.1.2",
  "sutra_position": "04.01.002"
}
```

## Claim template schemas

### sup_suffix

Suffix lookup for nominal declension.

```json
{
  "stem_class": "string — stem classification (e.g., 'a-stem-m', 'a-stem-n', 'i-stem-m')",
  "vibhakti": "string — case (prathama, dvitiya, tritiya, caturthi, pancami, sasthi, saptami, sambodhana)",
  "vacana": "string — number (ekavacana, dvivacana, bahuvacana)",
  "pratyaya": "string — raw sUP pratyaya with markers (e.g., 'su', 'ne', 'nasi')",
  "suffix": "string — processed suffix after marker removal (e.g., 's', 'e', 'as')",
  "markers": ["string — it-letters extracted from the pratyaya (e.g., ['n'])"],
  "sutra": "string",
  "sutra_position": "string"
}
```

### pratyaya_rule

Rules that modify the suffix based on stem class or phonological context. These are distinct from anga rules (which modify the stem) and from sandhi (which operates at the junction).

```json
{
  "condition_stem_class": "string|null — stem class this rule applies to",
  "condition_suffix": "string|null — which suffix this rule modifies",
  "condition_markers": ["string|null — marker conditions"],
  "input_suffix": "string — suffix before modification",
  "output_suffix": "string — suffix after modification",
  "sutra": "string",
  "sutra_position": "string",
  "rule_type": "string — utsarga|apavada|nitya|paribhasa"
}
```

### anga_rule

Rules that modify the stem (anga) before suffix attachment. Conditions may reference suffix markers, suffix phonological shape, or stem class.

```json
{
  "condition_stem_final": "string|null — phonological condition on stem ending",
  "condition_markers": ["string|null — pratyaya marker conditions (e.g., 'n' for nit suffixes)"],
  "condition_suffix_initial": "string|null — first phoneme of suffix",
  "operation": "string — what to do: 'lengthen', 'guna', 'vrddhi', 'substitute'",
  "operation_target": "string — what to modify: 'stem_final' (most common)",
  "operation_input": "string|null — specific input for substitute operations",
  "operation_output": "string|null — specific output for substitute operations",
  "sutra": "string",
  "sutra_position": "string",
  "rule_type": "string — utsarga|apavada|nitya|paribhasa"
}
```

### tripadi_rule

Post-combination phonological rules from the tripadi section (8.2-8.4). These are "asiddha" — they cannot trigger or block earlier rules.

```json
{
  "context": "string — phonological context description (e.g., 'word_final', 'after_vowel')",
  "condition_preceding": "string|null — what precedes the target",
  "condition_following": "string|null — what follows the target",
  "input": "string — phoneme(s) to match",
  "output": "string — replacement phoneme(s)",
  "position": "string — 'word_final'|'internal'|'any'",
  "sutra": "string",
  "sutra_position": "string",
  "rule_type": "string"
}
```

## Deva paradigm: complete derivation with sutra citations

All 24 forms of deva (a-stem masculine). Derivation traced through the five-layer pipeline using sutras from the Astadhyayi (source: SCL ashtadhyayi_simulator/aRt_new).

### Reference: raw sUP pratyayas (4-1-2)

su, au, jas, am, aut, sas, ta, bhyam, bhis, ne, bhyam, bhyas, nasi, bhyam, bhyas, nas, os, am, ni, os, sup

After anubandha removal: s, au, as, am, au, as, a, bhyam, bhis, e, bhyam, bhyas, as, bhyam, bhyas, as, os, am, i, os, su

### Derivation table

| # | vibhakti | vacana | form | pratyaya | L2: pratyaya mod | L3: anga mod | L4: sandhi | L5: tripadi |
|---|---|---|---|---|---|---|---|---|
| 1 | prathama | eka | devah | su->s | - | - | - | s->h (8.2.66) |
| 2 | prathama | dvi | devau | au | - | - | a+au->au (6.1.88) | - |
| 3 | prathama | bahu | devah | jas->as | - | - | a+a->a (6.1.101) | s->h (8.2.66) |
| 4 | dvitiya | eka | devam | am | - | - | - | - |
| 5 | dvitiya | dvi | devau | aut->au | - | - | a+au->au (6.1.88) | - |
| 6 | dvitiya | bahu | devan | sas | - | - | a+sas->an (6.1.103) | - |
| 7 | tritiya | eka | devena | ta->a | ta->ina (7.1.12) | - | a+i->e (6.1.87) | - |
| 8 | tritiya | dvi | devabhyam | bhyam | - | a->a (7.3.101/102) | - | - |
| 9 | tritiya | bahu | devaih | bhis | bhis->ais (7.1.9) | - | a+ai->ai (6.1.88) | s->h (8.2.66) |
| 10 | caturthi | eka | devaya | ne->e | ne->ya (7.1.13) | a->a (7.3.101/102) | - | - |
| 11 | caturthi | dvi | devabhyam | bhyam | - | a->a (7.3.101/102) | - | - |
| 12 | caturthi | bahu | devebhyah | bhyas | - | a->e (7.3.103) | - | s->h (8.2.66) |
| 13 | pancami | eka | devat | nasi->as | nasi->t (7.1.12) | a->a (7.3.101/102) | - | - |
| 14 | pancami | dvi | devabhyam | bhyam | - | a->a (7.3.101/102) | - | - |
| 15 | pancami | bahu | devebhyah | bhyas | - | a->e (7.3.103) | - | s->h (8.2.66) |
| 16 | sasthi | eka | devasya | nas->as | nas->sya (7.1.12) | - | - | - |
| 17 | sasthi | dvi | devayoh | os | - | a->e (7.3.104) | e+o->ay+o (6.1.78) | s->h (8.2.66) |
| 18 | sasthi | bahu | devanam | am | +nut (7.1.54) | a->a (7.3.101/102) | - | - |
| 19 | saptami | eka | deve | ni->i | - | - | a+i->e (6.1.87) | - |
| 20 | saptami | dvi | devayoh | os | - | a->e (7.3.104) | e+o->ay+o (6.1.78) | s->h (8.2.66) |
| 21 | saptami | bahu | devesu | sup->su | - | a->e (7.3.103) | - | s->s (8.3.59) |
| 22 | sambodhana | eka | deva | su->s | s->luk (6.1.69) | - | - | - |
| 23 | sambodhana | dvi | devau | au | - | - | a+au->au (6.1.88) | - |
| 24 | sambodhana | bahu | devah | jas->as | - | - | a+a->a (6.1.101) | s->h (8.2.66) |

### Sutra inventory for a-stem masculine

**Layer 2 -- pratyaya modification:**
- 7.1.9: atah bhis ais -- bhis->ais for a-stems
- 7.1.12: ta-nasi-nas-am ina-t-sya-h -- ta->ina, nasi->t, nas->sya for a-stems before vowel
- 7.1.13: neh yah -- ne->ya for a-stems
- 7.1.54: hrasvanadyapah nut -- nut augment before am (gen pl) after short vowel
- 6.1.69: enghrasvat sambuddheh -- luk (deletion) of su in vocative after short vowel
- 6.1.103: tasmat sasah nah pumsi -- sas->n for masculine after a

**Layer 3 -- anga modification:**
- 7.3.101: atah dirghah yani -- final a->a before yaÑ-initial
- 7.3.102: supi ca -- extends 7.3.101 to sUP context
- 7.3.103: bahuvacane jhali et -- a->e before jhaL-initial in plural
- 7.3.104: osi ca -- a->e before os (gen/loc dual)

**Layer 4 -- sandhi (mostly already seeded):**
- 6.1.87: ad gunah -- a + iK->guna (a+i->e, a+u->o, a+r->ar)
- 6.1.88: vrddhir eci -- a + eC->vrddhi (a+e->ai, a+o->au)
- 6.1.101: akah savarne dirghah -- a/a + savarna->dirgha
- 6.1.78: ecah ayavayavah -- e/o/ai/au->ay/av/ay/av before vowel (NEW — not in current seed data)

**Layer 5 -- tripadi:**
- 8.2.66: sasajusoh ruh -- word-final s->visarga
- 8.3.59: adesapratyayayoh -- s->s after i/u/r/e/o (in pratyaya)

### Notes on specific derivations

**devena (inst sg)**: 7.1.12 substitutes ta->ina, giving suffix "ina". Then a+i junction -> e by 6.1.87 (guna). Result: dev + e + na = devena.

**devaih (inst pl)**: 7.1.9 substitutes bhis->ais, giving suffix "ais". Then a+ai junction -> ai by 6.1.88 (vrddhi). Result: dev + ai + s -> devais -> devaih (8.2.66).

**devaya (dat sg)**: 7.1.13 substitutes ne->ya, giving suffix "ya". Then 7.3.101/102 lengthens stem-final a->a before ya (yaÑ-initial). Result: deva + ya = devaya.

**devayoh (gen/loc du)**: 7.3.104 changes stem-final a->e before os. Then 6.1.78 converts e before vowel (o) into ay. Result: dev + ay + os -> devayos -> devayoh (8.2.66).

**devan (acc pl)**: 6.1.103 replaces sas->n for masculine after a-stem. Then junction: a + n (no sandhi). Result: devan. Note: this is classified here as a sandhi-layer rule (6.1.x) but functionally it is a pratyaya substitution.

**devanam (gen pl)**: 7.1.54 adds nut augment before am, giving n+am = nam. Then 7.3.101/102 lengthens a->a before n (which is yaÑ? -- actually n is not yaÑ, so the lengthening may come from a different mechanism). Result: deva + nam = devanam. [Needs verification: the lengthening mechanism for gen pl may involve a different sutra.]

## Relationship to verbs

The five-layer pipeline generalizes to tinanta (verbal) derivation with one structural addition: verbs insert a vikarana (class marker) between root and suffix, creating a second junction point. This means the inner layers (pratyaya mod, anga, sandhi) would run twice — once for root+vikarana attachment, once for combined-stem+suffix attachment. Tripadi runs once at the end.

Additional claim templates for verbs (future, not part of this spike):
- `tin_suffix` — verbal suffix lookup by (lakara, purusha, vacana, pada)
- `vikarana` — class marker lookup by gana

Lakara (tense/mood) is metadata on the tin_suffix lookup, not a separate transformation.

The pipeline refactor from one-pass to iterative is ~50-100 lines of orchestration code. The claim templates and rule data are unaffected.

## Task decomposition

See yojana tasks created from this design.
