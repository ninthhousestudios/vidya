# SCL (Sanskrit Computational Linguistics) reference

## Location

`~/soft/scl` — GPL-2.0 licensed. Git repo from the Sanskrit Heritage / UoH project.

## Ashtadhyayi simulator

The most useful part for vidya is at `~/soft/scl/ashtadhyayi_simulator/june12/`.

### Sutra file

`aRt_new` — complete sutrapatha in WX transliteration. 3984 sutras. Format:

```
adhyaya-pada-sutra sutra-text-in-WX
```

Example:
```
6-1-87 Ax guNaH
7-1-12 tAfasifasAm inAwsyAH
8-2-66 sasajuRoH ruH
```

**Searching**: panda hooks break `grep` on this file (injects `--no-heading`). Use `awk` instead:

```bash
awk '/^7-1-12 /' ~/soft/scl/ashtadhyayi_simulator/june12/aRt_new
```

For multiple sutras:

```bash
awk '/^6-1-87 |^7-3-101 |^8-2-66 /' ~/soft/scl/ashtadhyayi_simulator/june12/aRt_new
```

### WX transliteration

WX is a lossless ASCII encoding for Devanagari/IAST. Key mappings:

| WX | IAST | WX | IAST |
|---|---|---|---|
| a | a | A | ā |
| i | i | I | ī |
| u | u | U | ū |
| q | ṛ | Q | ṝ |
| e | e | E | ai |
| o | o | O | au |
| k | k | K | kh |
| g | g | G | gh |
| f | ṅ | c | c |
| C | ch | j | j |
| J | jh | F | ñ |
| t | ṭ | T | ṭh |
| d | ḍ | D | ḍh |
| N | ṇ | w | t |
| W | th | x | d |
| X | dh | n | n |
| p | p | P | ph |
| b | b | B | bh |
| m | m | y | y |
| r | r | l | l |
| v | v | S | ś |
| R | ṣ | s | s |
| h | h | H | ḥ (visarga) |
| M | ṁ (anusvara) | | |

Note the non-obvious ones: `w`=t, `x`=d, `W`=th, `X`=dh, `f`=ṅ, `F`=ñ, `q`=ṛ.

### Rule implementations

`get_SabdarUpa_new.java` — the main simulator. Contains all rule instantiations as Java objects. Rules are categorized:

| Category | Java class | Astadhayi section |
|---|---|---|
| `afga_viXi` | `afga_viXi_rule` | Stem modification (adhyaya 6-7) |
| `prawyaya_viXi` | `prawyaya_viXi_rule` | Suffix modification (adhyaya 3-5) |
| `ekAxeSa` | (in rule class) | Single-substitute rules (6.1.x) |
| `wripAxI` | `wripAxI_rule` | Late-pass rules (8.2-8.4) |
| `sanXi` | `Sandhi_rule` | Sandhi (6.1.x) |

**Finding how a specific sutra is used**:

```bash
awk '/7-1-9/' ~/soft/scl/ashtadhyayi_simulator/june12/get_SabdarUpa_new.java
```

This shows the rule constructor call with: sutra number, category, stem regex, suffix regex, stem attributes, suffix attributes. Example output:

```java
list_rule[r]=new rule("7-1-9", "afga_viXi", "\\w*a$", "\\w*", "afga", "root(Bis)");r++;
```

Reading this: sutra 7-1-9, anga-vidhi category, applies when stem ends in `a`, any suffix, stem is an anga, and the suffix root is bhis (Bis in WX).

### Other useful files

| File | Content |
|---|---|
| `nouns` | Noun stems with gender classifications |
| `Xawu` | Verb roots (dhatu list) |
| `BARiwapuMs` | Bharita masculine stem data |
| `sarvanAma` | Pronoun stem data |
| `safKyA` | Numeral data |
| `rule_niyama` | Rule constraint/ordering data |

### The sUP pratyayas

The simulator defines them at line ~2800 of `get_SabdarUpa_new.java`:

```java
public static String [] viBakwi = {
    "su,1", "O", "jas", "am", "Ot", "Sas",
    "tA", "ByAm", "Bis", "fe", "ByAm", "Byas",
    "fasi,3", "ByAm", "Byas", "fas", "os", "Am",
    "fi", "os", "sup"
};
```

In IAST: su, au, jas, am, auṭ, śas, ṭā, bhyām, bhis, ṅe, bhyām, bhyas, ṅasi, bhyām, bhyas, ṅas, os, ām, ṅi, os, sup.

Ordered: prathamā-eka through saptamī-bahu (7 cases × 3 numbers = 21 suffixes). Sambodhana (vocative) reuses prathamā suffixes with special rules.

## Other SCL directories

| Path | Content |
|---|---|
| `sandhi/` | Sandhi splitting tools and data |
| `dhaatupaatha/` | Verb root database |
| `amarakosha/` | Amarakosha thesaurus data |
| `skt_gen/` | Sanskrit generator (OCaml) — compounds, sentences |
| `GOLD_DATA/` | Gold-standard test data for sandhi, compounds, parsing |
| `converters/` | Transliteration converters (WX, IAST, Devanagari, etc.) |

## How we used this for the declension design

In the session that produced `docs/declension-engine-design.md`, we:

1. Searched `aRt_new` for specific sutra numbers to get their text
2. Searched `get_SabdarUpa_new.java` for those same numbers to see how the simulator implements them (what conditions, what category)
3. Cross-referenced the simulator's rule categorization (afga_viXi, prawyaya_viXi, etc.) with our five-layer pipeline design
4. Used the `viBakwi` array to get the canonical sUP suffix list

This resolved all the "which sutra applies here?" questions for the deva paradigm derivation table.
