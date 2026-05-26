use super::matcher::ResolvedToken;
use super::vocab::SchemaVocab;

#[derive(Debug, Clone, Default)]
pub struct ProvenanceScope {
    pub tradition: Option<String>,
    pub source: Option<String>,
    pub pramana: Option<String>,
}

impl ProvenanceScope {
    pub fn is_empty(&self) -> bool {
        self.tradition.is_none() && self.source.is_none() && self.pramana.is_none()
    }
}

#[derive(Debug, Clone)]
pub enum ResolvedQuery {
    Describe {
        subject_iri: String,
    },
    Search {
        type_iri: String,
        filters: Vec<(String, String)>,
    },
    Traverse {
        subject_iri: String,
        predicate_iri: String,
    },
    Provenance {
        subject_iri: String,
        predicate_iri: String,
        object: String,
        object_is_literal: bool,
    },
    Similar {
        subject_iri: String,
    },
    Unbind {
        subject_iri: String,
        predicate_iri: String,
    },
}

#[derive(Debug, Clone)]
pub struct ResolutionReport {
    pub query: ResolvedQuery,
    pub scope: ProvenanceScope,
    pub unknown_tokens: Vec<String>,
    pub resolution_details: Vec<String>,
    pub alternatives: Vec<AlternativeParse>,
}

#[derive(Debug, Clone)]
pub struct AlternativeParse {
    pub query: ResolvedQuery,
    pub pattern_name: String,
    pub score: f64,
    pub score_breakdown: Vec<(String, f64)>,
}

#[derive(Debug, thiserror::Error)]
pub enum AssembleError {
    #[error("no entity found in input — did you mean one of: {}", candidates.join(", "))]
    NoEntity { candidates: Vec<String> },
    #[error("multiple entities found: {}; please be more specific", entities.join(", "))]
    AmbiguousEntity { entities: Vec<String> },
    #[error("no type found in input for search — need a type like 'graha' or 'rashi'")]
    NoType,
    #[error("multiple types match the given properties: {}; specify a type", candidates.join(", "))]
    AmbiguousType { candidates: Vec<String> },
    #[error("no predicate found in input for traverse — need a relation like 'rules' or 'exaltedIn'")]
    NoPredicate,
    #[error("provenance requires subject, predicate, and object — missing: {missing}")]
    IncompleteProv { missing: String },
    #[error("could not resolve any tokens from input")]
    NothingResolved,
}

#[derive(Debug, Clone, Copy)]
pub enum QueryMode {
    Describe,
    Search,
    Traverse,
    Provenance,
    Similar,
    Unbind,
}

pub fn assemble(
    mode: QueryMode,
    tokens: &[ResolvedToken],
    vocab: &SchemaVocab,
) -> std::result::Result<ResolutionReport, AssembleError> {
    let unknown_tokens: Vec<String> = tokens
        .iter()
        .filter_map(|t| match t {
            ResolvedToken::Unknown(s) => Some(s.clone()),
            _ => None,
        })
        .collect();

    let resolved_count = tokens.len() - unknown_tokens.len();
    if resolved_count == 0 && !tokens.is_empty() {
        return Err(AssembleError::NothingResolved);
    }

    match mode {
        QueryMode::Describe => assemble_describe(tokens, unknown_tokens),
        QueryMode::Search => assemble_search(tokens, unknown_tokens, vocab),
        QueryMode::Traverse => assemble_traverse(tokens, unknown_tokens),
        QueryMode::Provenance => assemble_provenance(tokens, unknown_tokens),
        QueryMode::Similar => assemble_similar(tokens, unknown_tokens),
        QueryMode::Unbind => assemble_unbind(tokens, unknown_tokens),
    }
}

fn assemble_describe(
    tokens: &[ResolvedToken],
    unknown_tokens: Vec<String>,
) -> std::result::Result<ResolutionReport, AssembleError> {
    let entities: Vec<&str> = tokens
        .iter()
        .filter_map(|t| match t {
            ResolvedToken::Entity { iri, .. } => Some(iri.as_str()),
            _ => None,
        })
        .collect();

    match entities.len() {
        0 => Err(AssembleError::NoEntity {
            candidates: suggest_from_tokens(tokens),
        }),
        1 => Ok(ResolutionReport {
            query: ResolvedQuery::Describe {
                subject_iri: entities[0].to_string(),
            },
            unknown_tokens,
            resolution_details: vec![format!("subject: {}", short_name(entities[0]))],
            scope: ProvenanceScope::default(),
            alternatives: Vec::new(),
        }),
        _ => {
            let deduped: Vec<&str> = dedup_strs(&entities);
            if deduped.len() == 1 {
                return Ok(ResolutionReport {
                    query: ResolvedQuery::Describe {
                        subject_iri: deduped[0].to_string(),
                    },
                    unknown_tokens,
                    resolution_details: vec![format!("subject: {}", short_name(deduped[0]))],
                    scope: ProvenanceScope::default(),
            alternatives: Vec::new(),
                });
            }
            Err(AssembleError::AmbiguousEntity {
                entities: deduped.iter().map(|e| short_name(e)).collect(),
            })
        }
    }
}

fn assemble_search(
    tokens: &[ResolvedToken],
    unknown_tokens: Vec<String>,
    vocab: &SchemaVocab,
) -> std::result::Result<ResolutionReport, AssembleError> {
    let types: Vec<&str> = tokens
        .iter()
        .filter_map(|t| match t {
            ResolvedToken::Type { iri, .. } => Some(iri.as_str()),
            _ => None,
        })
        .collect();

    let prop_values: Vec<(&str, &str)> = tokens
        .iter()
        .filter_map(|t| match t {
            ResolvedToken::PropertyValue {
                predicate_local,
                value,
                ..
            } => Some((predicate_local.as_str(), value.as_str())),
            _ => None,
        })
        .collect();

    let (type_iri, inferred) = if let Some(t) = types.first() {
        (t.to_string(), false)
    } else if !prop_values.is_empty() {
        match infer_type_from_values(&prop_values, vocab) {
            Ok(iri) => (iri, true),
            Err(e) => return Err(e),
        }
    } else {
        return Err(AssembleError::NoType);
    };

    let filters: Vec<(String, String)> = prop_values
        .iter()
        .map(|(pred, val)| (short_name(pred), val.to_string()))
        .collect();

    let type_short = short_name(&type_iri);
    let mut details = if inferred {
        vec![format!("type: {type_short} (inferred from property values)")]
    } else {
        vec![format!("type: {type_short}")]
    };
    for (k, v) in &filters {
        details.push(format!("filter: {k}={v}"));
    }

    Ok(ResolutionReport {
        query: ResolvedQuery::Search {
            type_iri,
            filters,
        },
        unknown_tokens,
        resolution_details: details,
        scope: ProvenanceScope::default(),
        alternatives: Vec::new(),
    })
}

fn infer_type_from_values(
    prop_values: &[(&str, &str)],
    vocab: &SchemaVocab,
) -> std::result::Result<String, AssembleError> {
    let mut candidate_sets: Vec<Vec<&str>> = Vec::new();
    for (pred, val) in prop_values {
        let types = vocab.types_for_value(pred, val);
        if !types.is_empty() {
            candidate_sets.push(types.iter().map(|s| s.as_str()).collect());
        }
    }

    if candidate_sets.is_empty() {
        return Err(AssembleError::NoType);
    }

    let mut candidates: Vec<&str> = candidate_sets[0].clone();
    for set in &candidate_sets[1..] {
        candidates.retain(|c| set.contains(c));
    }

    match candidates.len() {
        0 => Err(AssembleError::NoType),
        1 => Ok(candidates[0].to_string()),
        _ => Err(AssembleError::AmbiguousType {
            candidates: candidates.iter().map(|c| short_name(c)).collect(),
        }),
    }
}

fn assemble_traverse(
    tokens: &[ResolvedToken],
    unknown_tokens: Vec<String>,
) -> std::result::Result<ResolutionReport, AssembleError> {
    let entities: Vec<&str> = tokens
        .iter()
        .filter_map(|t| match t {
            ResolvedToken::Entity { iri, .. } => Some(iri.as_str()),
            _ => None,
        })
        .collect();

    let predicates: Vec<&str> = tokens
        .iter()
        .filter_map(|t| match t {
            ResolvedToken::Predicate { iri, .. } => Some(iri.as_str()),
            _ => None,
        })
        .collect();

    let subject_iri = match dedup_strs(&entities).first() {
        Some(e) => e.to_string(),
        None => {
            return Err(AssembleError::NoEntity {
                candidates: suggest_from_tokens(tokens),
            })
        }
    };

    let predicate_iri = match predicates.first() {
        Some(p) => p.to_string(),
        None => {
            // Try to fuzzy-match predicates from property value tokens or unknowns
            return Err(AssembleError::NoPredicate);
        }
    };

    Ok(ResolutionReport {
        query: ResolvedQuery::Traverse {
            subject_iri: subject_iri.clone(),
            predicate_iri: predicate_iri.clone(),
        },
        unknown_tokens,
        resolution_details: vec![
            format!("subject: {}", short_name(&subject_iri)),
            format!("predicate: {}", short_name(&predicate_iri)),
        ],
        scope: ProvenanceScope::default(),
        alternatives: Vec::new(),
    })
}

fn assemble_provenance(
    tokens: &[ResolvedToken],
    unknown_tokens: Vec<String>,
) -> std::result::Result<ResolutionReport, AssembleError> {
    let entities: Vec<&str> = tokens
        .iter()
        .filter_map(|t| match t {
            ResolvedToken::Entity { iri, .. } => Some(iri.as_str()),
            _ => None,
        })
        .collect();

    let predicates: Vec<&str> = tokens
        .iter()
        .filter_map(|t| match t {
            ResolvedToken::Predicate { iri, .. } => Some(iri.as_str()),
            _ => None,
        })
        .collect();

    let prop_values: Vec<(&str, &str)> = tokens
        .iter()
        .filter_map(|t| match t {
            ResolvedToken::PropertyValue { value, .. } => Some(("", value.as_str())),
            _ => None,
        })
        .collect();

    let mut missing = Vec::new();

    let subject_iri = if let Some(e) = entities.first() {
        e.to_string()
    } else {
        missing.push("subject");
        String::new()
    };

    let predicate_iri = if let Some(p) = predicates.first() {
        p.to_string()
    } else {
        missing.push("predicate");
        String::new()
    };

    // Object can be second entity or a property value
    let (object, object_is_literal) = if entities.len() >= 2 {
        (entities[1].to_string(), false)
    } else if let Some((_, val)) = prop_values.first() {
        (val.to_string(), true)
    } else {
        missing.push("object");
        (String::new(), false)
    };

    if !missing.is_empty() {
        return Err(AssembleError::IncompleteProv {
            missing: missing.join(", "),
        });
    }

    Ok(ResolutionReport {
        query: ResolvedQuery::Provenance {
            subject_iri: subject_iri.clone(),
            predicate_iri: predicate_iri.clone(),
            object: object.clone(),
            object_is_literal,
        },
        unknown_tokens,
        resolution_details: vec![
            format!("subject: {}", short_name(&subject_iri)),
            format!("predicate: {}", short_name(&predicate_iri)),
            format!(
                "object: {}",
                if object_is_literal {
                    format!("\"{object}\"")
                } else {
                    short_name(&object)
                }
            ),
        ],
        scope: ProvenanceScope::default(),
        alternatives: Vec::new(),
    })
}

fn assemble_similar(
    tokens: &[ResolvedToken],
    unknown_tokens: Vec<String>,
) -> std::result::Result<ResolutionReport, AssembleError> {
    let entities: Vec<&str> = tokens
        .iter()
        .filter_map(|t| match t {
            ResolvedToken::Entity { iri, .. } => Some(iri.as_str()),
            _ => None,
        })
        .collect();

    match dedup_strs(&entities).as_slice() {
        [] => Err(AssembleError::NoEntity {
            candidates: suggest_from_tokens(tokens),
        }),
        [iri] => Ok(ResolutionReport {
            query: ResolvedQuery::Similar {
                subject_iri: iri.to_string(),
            },
            unknown_tokens,
            resolution_details: vec![format!("subject: {}", short_name(iri))],
            scope: ProvenanceScope::default(),
            alternatives: Vec::new(),
        }),
        iris => Err(AssembleError::AmbiguousEntity {
            entities: iris.iter().map(|e| short_name(e)).collect(),
        }),
    }
}

fn assemble_unbind(
    tokens: &[ResolvedToken],
    unknown_tokens: Vec<String>,
) -> std::result::Result<ResolutionReport, AssembleError> {
    let entities: Vec<&str> = tokens
        .iter()
        .filter_map(|t| match t {
            ResolvedToken::Entity { iri, .. } => Some(iri.as_str()),
            _ => None,
        })
        .collect();

    let predicates: Vec<&str> = tokens
        .iter()
        .filter_map(|t| match t {
            ResolvedToken::Predicate { iri, .. } => Some(iri.as_str()),
            _ => None,
        })
        .collect();

    let subject_iri = match dedup_strs(&entities).first() {
        Some(e) => e.to_string(),
        None => {
            return Err(AssembleError::NoEntity {
                candidates: suggest_from_tokens(tokens),
            })
        }
    };

    let predicate_iri = match predicates.first() {
        Some(p) => p.to_string(),
        None => return Err(AssembleError::NoPredicate),
    };

    Ok(ResolutionReport {
        query: ResolvedQuery::Unbind {
            subject_iri: subject_iri.clone(),
            predicate_iri: predicate_iri.clone(),
        },
        unknown_tokens,
        resolution_details: vec![
            format!("subject: {}", short_name(&subject_iri)),
            format!("predicate: {}", short_name(&predicate_iri)),
        ],
        scope: ProvenanceScope::default(),
        alternatives: Vec::new(),
    })
}

pub(crate) fn short_name(iri: &str) -> String {
    iri.rsplit_once('/')
        .map(|(_, local)| local.to_string())
        .unwrap_or_else(|| iri.to_string())
}

fn suggest_from_tokens(tokens: &[ResolvedToken]) -> Vec<String> {
    tokens
        .iter()
        .filter_map(|t| match t {
            ResolvedToken::Type { iri, .. } => Some(short_name(iri)),
            ResolvedToken::Predicate { iri, .. } => Some(short_name(iri)),
            ResolvedToken::PropertyValue { value, .. } => Some(value.clone()),
            _ => None,
        })
        .collect()
}

fn dedup_strs<'a>(strs: &[&'a str]) -> Vec<&'a str> {
    let mut v: Vec<&str> = strs.to_vec();
    v.sort();
    v.dedup();
    v
}
