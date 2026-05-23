# Vidya

A provenance-tracking knowledge store for traditional Indian knowledge systems. Stores domain knowledge as RDF triples with attestation metadata (who said it, from which text, via which means of knowing, with what confidence).

## Language — Base Ontology

**Pramana**:
A means of valid knowledge (epistemological category). Six types: pratyaksha (perception), anumana (inference), shabda (authoritative testimony), upamana (analogy), arthapatti (presumption), anupalabdhi (non-apprehension).
_Avoid_: evidence type, proof type

**Tradition**:
A lineage of knowledge transmission. Has parent-child hierarchy.
_Avoid_: school (ambiguous), sect

**Source**:
A specific text or authority from which an assertion is drawn. Belongs to a tradition.
_Avoid_: reference, citation

**Assertion**:
Provenance metadata attached to a triple via RDF-star annotation. Records tradition, source, pramana, and confidence.
_Avoid_: claim, annotation (too generic)

**Vipratipatti** (deferred):
Conflicting testimony across sources — when multiple shabda pramanas assert contradictory claims. Not yet modeled as a first-class entity; disagreements are currently implicit (visible by comparing assertion sources on the same subject+property). Planned as a future first-class entity for discovery and resolution tracking.

## Language — Ayurveda Domain

**Dravya**:
A substance used in ayurvedic medicine. The central entity in dravyaguna (pharmacology).
_Avoid_: herb, drug, medicine (too narrow)

**Rasa**:
Taste. Exactly 6: madhura (sweet), amla (sour), lavana (salty), katu (pungent), tikta (bitter), kashaya (astringent). Also serves as the value set for vipaka.
_Avoid_: flavor

**Guna**:
Quality/attribute. 20 canonical gunas in 10 opposing pairs (guru/laghu, sheeta/ushna, snigdha/ruksha, etc.). Pairs are linked via `oppositeGuna`. Also serves as the value set for veerya.
_Avoid_: property (overloaded), attribute (too generic)

**Veerya**:
Potency — the dominant active quality of a substance. Not a separate entity type; modeled as gunas assigned via `hasVeerya`. Charaka's 8-fold system selects 8 gunas as potential veerya values; Sushruta's 2-fold system allows only sheeta and ushna. This disagreement is captured via per-source provenance on `hasVeerya` triples.
_Avoid_: potency (English approximation is fine in prose but not as a term)

**Vipaka**:
Post-digestive effect — the rasa that emerges after digestion. Not a separate entity type; modeled as rasas assigned via `hasVipaka`. Only 3 values: madhura, amla, katu.
_Avoid_: post-digestive taste

**Dosha**:
Bio-energetic principle. Exactly 3: vata (air+ether), pitta (fire+water), kapha (water+earth). Dravyas relate to doshas via `pacifiesDosha` and `aggravatesDosha`.
_Avoid_: humor, constitution (that's prakriti)

**Karma**:
Therapeutic action/effect of a substance (e.g., deepana, rasayana, jvaraghna). Flat enumeration, defined as-needed per dravya. Not to be confused with jyotish Karma Bhava (10th house).
_Avoid_: action (too generic), effect

## Relationships

- A **Dravya** has one or more **Rasa** values (via `hasRasa`)
- A **Dravya** has one or more **Guna** values (via `hasGuna`)
- A **Dravya** has a **Veerya** (via `hasVeerya`, range is **Guna**)
- A **Dravya** has a **Vipaka** (via `hasVipaka`, range is **Rasa**)
- A **Dravya** has one or more **Karma** effects (via `hasKarma`)
- A **Dravya** pacifies or aggravates one or more **Dosha** values
- A **Rasa** pacifies or aggravates **Dosha** values (theory-level triples)
- A **Guna** has an opposite **Guna** (via `oppositeGuna`, symmetric)
- Every relation can carry an **Assertion** via RDF-star, linking to **Source**, **Tradition**, and **Pramana**

## Ayurveda Traditions and Sources

- **Atreya** tradition (Atreya-Punarvasu lineage) → source: Charaka Samhita
- **Dhanvantari** tradition (surgical lineage) → source: Sushruta Samhita
- **Bhavaprakasha Nighantu** is a source (not a tradition) under the parent ayurveda tradition — it's a later synthesis, not a distinct school

## Naming Conventions

- **IRIs**: lowercase ASCII, no diacritics (`ayurveda:ashwagandha`, `ayurveda:madhura`)
- **Labels**: untagged `rdfs:label` for the primary name (Sanskrit IAST form, e.g. `"Aśvagandha"`). English common names via a separate property or alias. Language-tagged labels deferred until the query engine supports multi-label deduplication.
- **Botanical names**: dedicated `ayurveda:botanicalName` property (not a label variant)

## Example dialogue

> **Dev:** "When we say ashwagandha has veerya ushna, is that a separate entity from guna ushna?"
> **Domain expert:** "No — veerya IS a guna. The `hasVeerya` property points to the same ushna entity as `hasGuna`. Veerya just means 'the dominant active quality.'"

> **Dev:** "Charaka lists 8 veeryas but Sushruta only lists 2. How do we model that?"
> **Domain expert:** "Both use the same guna entities. Charaka's assertions attach 8 different gunas via `hasVeerya`; Sushruta's only ever attach sheeta or ushna. The provenance on each triple tells you which system you're looking at."

> **Dev:** "If a dravya pacifies vata, is that stored on the dravya directly or derived from its rasa?"
> **Domain expert:** "Both. The dravya has `pacifiesDosha vata` directly, and separately the rasa→dosha theory triples exist (e.g., madhura pacifies vata). The direct assertion is the fact; the theory triples explain why."

> **Dev:** "Why not use `rdfs:label` with `@sa` and `@en` language tags?"
> **Domain expert:** "The query engine doesn't deduplicate multi-label entities yet — search returns one hit per label, so every dravya would appear twice. Untagged labels for now; language tags when the engine supports them."

## Flagged ambiguities

- **Karma**: means "therapeutic action" in ayurveda but "10th house / career" in jyotish. Separate domain namespaces (`ayurveda:` vs `jyotish:`) resolve the collision.
- **Guna**: means "quality/attribute" in ayurveda but "quality of a rashi" (movable/fixed/dual) in jyotish. Same resolution — namespace separation.
- **Veerya/Vipaka are not separate entity types** despite being listed as "entity types" in the original task description. They reuse Guna and Rasa entities respectively, with distinct properties (`hasVeerya`, `hasVipaka`) to mark the role.
