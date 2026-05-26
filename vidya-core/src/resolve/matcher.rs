use super::vocab::SchemaVocab;
use crate::vsa::{EntityIndex, Hrr};

#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedToken {
    Entity {
        iri: String,
        confidence: MatchConfidence,
    },
    Type {
        iri: String,
        confidence: MatchConfidence,
    },
    Predicate {
        iri: String,
        confidence: MatchConfidence,
    },
    PropertyValue {
        predicate_local: String,
        value: String,
        confidence: MatchConfidence,
    },
    Unknown(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchConfidence {
    Exact,
    Substring,
    EditDistance,
    Vsa,
}

pub(crate) const STOPWORDS: &[&str] = &[
    "a", "an", "the", "in", "of", "is", "are", "was", "were", "what", "which", "who", "that",
    "this", "for", "with", "from", "by", "to", "and", "or", "not", "do", "does", "did", "has",
    "have", "had", "be", "been", "being", "it", "its", "my", "me", "about", "tell",
];

const MAX_EDIT_DISTANCE: usize = 2;
const VSA_THRESHOLD: f64 = 0.15;

pub fn tokenize(input: &str) -> Vec<String> {
    input
        .split_whitespace()
        .map(|t| {
            t.chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|t| !t.is_empty() && !STOPWORDS.contains(&t.as_str()))
        .collect()
}

pub fn match_tokens(
    tokens: &[String],
    vocab: &SchemaVocab,
    vsa: Option<&EntityIndex<Hrr>>,
    domain: &str,
) -> Vec<ResolvedToken> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if i + 1 < tokens.len() {
            let bigram = format!("{} {}", tokens[i], tokens[i + 1]);
            if let Some(r) = try_exact(&bigram, vocab) {
                result.push(r);
                i += 2;
                continue;
            }
        }
        result.push(match_single(&tokens[i], vocab, vsa, domain));
        i += 1;
    }
    result
}

fn match_single(
    token: &str,
    vocab: &SchemaVocab,
    vsa: Option<&EntityIndex<Hrr>>,
    domain: &str,
) -> ResolvedToken {
    if let Some(r) = try_exact(token, vocab) {
        return r;
    }
    if let Some(r) = try_substring(token, vocab) {
        return r;
    }
    if let Some(r) = try_edit_distance(token, vocab) {
        return r;
    }
    if let Some(vsa_index) = vsa {
        if let Some(r) = try_vsa(token, vocab, vsa_index, domain) {
            return r;
        }
    }
    ResolvedToken::Unknown(token.to_string())
}

fn try_exact(token: &str, vocab: &SchemaVocab) -> Option<ResolvedToken> {
    // Priority: entity > type > predicate > property value
    // Entities first because aliases/western names are the most specific identifiers.
    // Types before predicates because "graha" should be a type, not a predicate.
    // Property values last because they're the most ambiguous.
    if let Some(iris) = vocab.entity_names.get(token) {
        if let Some(iri) = iris.first() {
            return Some(ResolvedToken::Entity {
                iri: iri.clone(),
                confidence: MatchConfidence::Exact,
            });
        }
    }
    if let Some(iri) = vocab.type_names.get(token) {
        return Some(ResolvedToken::Type {
            iri: iri.clone(),
            confidence: MatchConfidence::Exact,
        });
    }
    if let Some(iri) = vocab.predicate_names.get(token) {
        return Some(ResolvedToken::Predicate {
            iri: iri.clone(),
            confidence: MatchConfidence::Exact,
        });
    }
    if let Some(entries) = vocab.value_index.get(token) {
        if let Some((pred_iri, val)) = entries.first() {
            return Some(ResolvedToken::PropertyValue {
                predicate_local: pred_iri.clone(),
                value: val.clone(),
                confidence: MatchConfidence::Exact,
            });
        }
    }
    None
}

fn try_substring(token: &str, vocab: &SchemaVocab) -> Option<ResolvedToken> {
    // Only try substring for tokens >= 3 chars to avoid spurious matches
    if token.len() < 3 {
        return None;
    }
    for (name, iris) in &vocab.entity_names {
        if name.contains(token) || token.contains(name.as_str()) {
            if let Some(iri) = iris.first() {
                return Some(ResolvedToken::Entity {
                    iri: iri.clone(),
                    confidence: MatchConfidence::Substring,
                });
            }
        }
    }
    for (name, iri) in &vocab.type_names {
        if name.contains(token) || token.contains(name.as_str()) {
            return Some(ResolvedToken::Type {
                iri: iri.clone(),
                confidence: MatchConfidence::Substring,
            });
        }
    }
    for (name, iri) in &vocab.predicate_names {
        if name.contains(token) || token.contains(name.as_str()) {
            return Some(ResolvedToken::Predicate {
                iri: iri.clone(),
                confidence: MatchConfidence::Substring,
            });
        }
    }
    None
}

fn try_edit_distance(token: &str, vocab: &SchemaVocab) -> Option<ResolvedToken> {
    let all_tokens = vocab.all_known_tokens();
    let mut best: Option<(String, usize)> = None;

    for candidate in &all_tokens {
        let dist = edit_distance(token, candidate);
        if dist <= MAX_EDIT_DISTANCE {
            if best.as_ref().is_none_or(|(_, d)| dist < *d) {
                best = Some((candidate.clone(), dist));
            }
        }
    }

    let best_key = best?.0;

    if let Some(iris) = vocab.entity_names.get(&best_key) {
        if let Some(iri) = iris.first() {
            return Some(ResolvedToken::Entity {
                iri: iri.clone(),
                confidence: MatchConfidence::EditDistance,
            });
        }
    }
    if let Some(iri) = vocab.type_names.get(&best_key) {
        return Some(ResolvedToken::Type {
            iri: iri.clone(),
            confidence: MatchConfidence::EditDistance,
        });
    }
    if let Some(iri) = vocab.predicate_names.get(&best_key) {
        return Some(ResolvedToken::Predicate {
            iri: iri.clone(),
            confidence: MatchConfidence::EditDistance,
        });
    }
    if let Some(entries) = vocab.value_index.get(&best_key) {
        if let Some((pred_iri, val)) = entries.first() {
            return Some(ResolvedToken::PropertyValue {
                predicate_local: pred_iri.clone(),
                value: val.clone(),
                confidence: MatchConfidence::EditDistance,
            });
        }
    }
    None
}

fn try_vsa(
    token: &str,
    vocab: &SchemaVocab,
    vsa_index: &EntityIndex<Hrr>,
    domain: &str,
) -> Option<ResolvedToken> {
    let token_iri = crate::ontology::domain_iri(domain, token);
    let results = vsa_index.similar(&token_iri, 1);
    if let Some((iri, sim)) = results.first() {
        if *sim > VSA_THRESHOLD {
            // Check if this IRI is a known entity
            let local = iri.rsplit_once('/').map(|(_, l)| l.to_lowercase());
            if let Some(ref local_name) = local {
                if vocab.entity_names.values().any(|iris| iris.contains(iri)) {
                    return Some(ResolvedToken::Entity {
                        iri: iri.clone(),
                        confidence: MatchConfidence::Vsa,
                    });
                }
                if vocab.type_names.values().any(|t| t == iri) {
                    return Some(ResolvedToken::Type {
                        iri: iri.clone(),
                        confidence: MatchConfidence::Vsa,
                    });
                }
                let _ = local_name;
            }
        }
    }
    None
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let m = a.len();
    let n = b.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_strips_stopwords_and_punctuation() {
        let tokens = tokenize("tell me about the Mars!");
        assert_eq!(tokens, vec!["mars"]);
    }

    #[test]
    fn tokenize_preserves_hyphens() {
        let tokens = tokenize("bhumi-putra");
        assert_eq!(tokens, vec!["bhumi-putra"]);
    }

    #[test]
    fn edit_distance_basic() {
        assert_eq!(edit_distance("mars", "mars"), 0);
        assert_eq!(edit_distance("mars", "mras"), 2);
        assert_eq!(edit_distance("mars", "mar"), 1);
        assert_eq!(edit_distance("", "abc"), 3);
    }

    #[test]
    fn tokenize_preserves_words_for_bigram() {
        let tokens = tokenize("1st House");
        assert_eq!(tokens, vec!["1st", "house"]);
    }

    #[test]
    fn match_tokens_bigram_entity() {
        use std::collections::HashMap;

        let mut entity_names = HashMap::new();
        entity_names.insert("1st house".to_string(), vec!["urn:bhava-1".to_string()]);

        let vocab = SchemaVocab {
            entity_names,
            type_names: HashMap::new(),
            predicate_names: HashMap::new(),
            value_index: HashMap::new(),
            value_types: HashMap::new(),
            tradition_names: HashMap::new(),
            source_names: HashMap::new(),
            pramana_names: HashMap::new(),
        };

        let tokens = tokenize("1st House");
        let matched = match_tokens(&tokens, &vocab, None, "test");
        assert_eq!(matched.len(), 1);
        assert_eq!(
            matched[0],
            ResolvedToken::Entity {
                iri: "urn:bhava-1".to_string(),
                confidence: MatchConfidence::Exact,
            }
        );
    }

    #[test]
    fn match_tokens_bigram_miss_falls_through() {
        use std::collections::HashMap;

        let mut entity_names = HashMap::new();
        entity_names.insert("mars".to_string(), vec!["urn:mangala".to_string()]);

        let vocab = SchemaVocab {
            entity_names,
            type_names: HashMap::new(),
            predicate_names: HashMap::new(),
            value_index: HashMap::new(),
            value_types: HashMap::new(),
            tradition_names: HashMap::new(),
            source_names: HashMap::new(),
            pramana_names: HashMap::new(),
        };

        let tokens = tokenize("mars rules");
        let matched = match_tokens(&tokens, &vocab, None, "test");
        assert_eq!(matched.len(), 2);
        assert_eq!(
            matched[0],
            ResolvedToken::Entity {
                iri: "urn:mangala".to_string(),
                confidence: MatchConfidence::Exact,
            }
        );
        matches!(&matched[1], ResolvedToken::Unknown(_));
    }
}
