use super::assemble::{QueryMode, ResolutionReport};
use super::intent::{IntentResult, ScopeCategory};
use super::matcher::{MatchConfidence, ResolvedToken};

#[derive(Debug, Clone)]
pub struct ScoreSignal {
    pub name: &'static str,
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct ScoredCandidate {
    pub report: ResolutionReport,
    pub pattern_name: &'static str,
    pub scope_hint: Option<String>,
    pub scope_category: ScopeCategory,
    pub total_score: f64,
    pub signals: Vec<ScoreSignal>,
}

pub(crate) struct ParseAttempt {
    pub intent: IntentResult,
    pub tokens: Vec<ResolvedToken>,
    pub report: ResolutionReport,
}

pub(crate) fn rank(attempts: Vec<ParseAttempt>) -> Vec<ScoredCandidate> {
    let mut candidates: Vec<ScoredCandidate> = attempts
        .into_iter()
        .map(|a| {
            let (total_score, signals) = score(&a.intent, &a.tokens, &a.report);
            ScoredCandidate {
                report: a.report,
                pattern_name: a.intent.pattern_name,
                scope_hint: a.intent.scope_hint,
                scope_category: a.intent.scope_category,
                total_score,
                signals,
            }
        })
        .collect();

    candidates.sort_by(|a, b| {
        b.total_score
            .partial_cmp(&a.total_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.pattern_name.cmp(&b.pattern_name))
    });
    candidates
}

fn score(
    intent: &IntentResult,
    tokens: &[ResolvedToken],
    report: &ResolutionReport,
) -> (f64, Vec<ScoreSignal>) {
    let mut signals = Vec::new();

    let total = tokens.len().max(1);
    let resolved = tokens
        .iter()
        .filter(|t| !matches!(t, ResolvedToken::Unknown(_)))
        .count();
    let coverage = resolved as f64 / total as f64;
    signals.push(ScoreSignal {
        name: "token_coverage",
        value: coverage * 0.30,
    });

    let tier_sum: f64 = tokens
        .iter()
        .filter_map(|t| {
            confidence_of(t).map(|c| match c {
                MatchConfidence::Exact => 1.0,
                MatchConfidence::Substring => 0.7,
                MatchConfidence::EditDistance => 0.4,
                MatchConfidence::Vsa => 0.2,
            })
        })
        .sum();
    let tier_avg = if resolved > 0 {
        tier_sum / resolved as f64
    } else {
        0.0
    };
    signals.push(ScoreSignal {
        name: "match_tier",
        value: tier_avg * 0.25,
    });

    signals.push(ScoreSignal {
        name: "no_unknowns",
        value: if report.unknown_tokens.is_empty() {
            0.10
        } else {
            0.0
        },
    });

    signals.push(ScoreSignal {
        name: "shape_validity",
        value: shape_validity(intent, tokens),
    });

    signals.push(ScoreSignal {
        name: "scope",
        value: if intent.scope_hint.is_some() {
            0.05
        } else {
            0.0
        },
    });

    signals.push(ScoreSignal {
        name: "pattern_specificity",
        value: pattern_specificity(intent.pattern_name),
    });

    let total_score: f64 = signals.iter().map(|s| s.value).sum();
    (total_score, signals)
}

fn confidence_of(token: &ResolvedToken) -> Option<MatchConfidence> {
    match token {
        ResolvedToken::Entity { confidence, .. } => Some(*confidence),
        ResolvedToken::Type { confidence, .. } => Some(*confidence),
        ResolvedToken::Predicate { confidence, .. } => Some(*confidence),
        ResolvedToken::PropertyValue { confidence, .. } => Some(*confidence),
        ResolvedToken::Unknown(_) => None,
    }
}

fn shape_validity(intent: &IntentResult, tokens: &[ResolvedToken]) -> f64 {
    let has_entity = tokens
        .iter()
        .any(|t| matches!(t, ResolvedToken::Entity { .. }));
    let has_predicate = tokens
        .iter()
        .any(|t| matches!(t, ResolvedToken::Predicate { .. }));
    let has_type = tokens
        .iter()
        .any(|t| matches!(t, ResolvedToken::Type { .. }));
    let has_prop_value = tokens
        .iter()
        .any(|t| matches!(t, ResolvedToken::PropertyValue { .. }));

    match intent.mode {
        QueryMode::Describe => {
            if has_entity && !has_predicate {
                0.20
            } else if has_entity {
                0.10
            } else {
                0.05
            }
        }
        QueryMode::Traverse => {
            if has_entity && has_predicate {
                0.20
            } else if has_entity {
                0.05
            } else {
                0.0
            }
        }
        QueryMode::Search => {
            if has_type && has_prop_value {
                0.20
            } else if has_type || has_prop_value {
                0.10
            } else {
                0.05
            }
        }
        QueryMode::Similar => {
            if has_entity {
                0.20
            } else {
                0.05
            }
        }
        QueryMode::Provenance => {
            if has_entity && has_predicate {
                0.15
            } else {
                0.05
            }
        }
        QueryMode::Unbind => {
            if has_entity && has_predicate {
                0.20
            } else {
                0.05
            }
        }
    }
}

fn pattern_specificity(pattern_name: &str) -> f64 {
    match pattern_name {
        "tradition_say" | "tradition_according" | "pramana_from" => 0.10,
        "traverse_possessive" => 0.09,
        "search_where" => 0.08,
        "search_what_are" => 0.07,
        "traverse_does" => 0.06,
        "similar" => 0.05,
        "traverse_what_is" => 0.04,
        "describe_tell" => 0.03,
        "describe_explicit" => 0.03,
        "describe_what_is" => 0.01,
        _ => 0.02,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traverse_entity_predicate_beats_describe() {
        let tokens = vec![
            ResolvedToken::Entity {
                iri: "urn:mangala".to_string(),
                confidence: MatchConfidence::Exact,
            },
            ResolvedToken::Predicate {
                iri: "urn:exaltedIn".to_string(),
                confidence: MatchConfidence::Exact,
            },
        ];

        let traverse_intent = IntentResult {
            mode: QueryMode::Traverse,
            slot_text: "mars exalted".to_string(),
            scope_hint: None,
            scope_category: ScopeCategory::Unknown,
            pattern_name: "traverse_what_is",
        };
        let describe_intent = IntentResult {
            mode: QueryMode::Describe,
            slot_text: "mars exalted".to_string(),
            scope_hint: None,
            scope_category: ScopeCategory::Unknown,
            pattern_name: "describe_what_is",
        };

        let traverse_report = ResolutionReport {
            query: super::super::assemble::ResolvedQuery::Traverse {
                subject_iri: "urn:mangala".to_string(),
                predicate_iri: "urn:exaltedIn".to_string(),
            },
            unknown_tokens: vec![],
            resolution_details: vec![],
            scope: super::super::assemble::ProvenanceScope::default(),
            alternatives: vec![],
        };
        let describe_report = ResolutionReport {
            query: super::super::assemble::ResolvedQuery::Describe {
                subject_iri: "urn:mangala".to_string(),
            },
            unknown_tokens: vec![],
            resolution_details: vec![],
            scope: super::super::assemble::ProvenanceScope::default(),
            alternatives: vec![],
        };

        let (traverse_score, _) = score(&traverse_intent, &tokens, &traverse_report);
        let (describe_score, _) = score(&describe_intent, &tokens, &describe_report);

        assert!(
            traverse_score > describe_score,
            "traverse ({traverse_score:.3}) should beat describe ({describe_score:.3})"
        );
    }

    #[test]
    fn describe_entity_only_beats_traverse_entity_only() {
        let tokens = vec![ResolvedToken::Entity {
            iri: "urn:mangala".to_string(),
            confidence: MatchConfidence::Exact,
        }];

        let traverse_intent = IntentResult {
            mode: QueryMode::Traverse,
            slot_text: "mars".to_string(),
            scope_hint: None,
            scope_category: ScopeCategory::Unknown,
            pattern_name: "traverse_what_is",
        };
        let describe_intent = IntentResult {
            mode: QueryMode::Describe,
            slot_text: "mars".to_string(),
            scope_hint: None,
            scope_category: ScopeCategory::Unknown,
            pattern_name: "describe_what_is",
        };

        let report = ResolutionReport {
            query: super::super::assemble::ResolvedQuery::Describe {
                subject_iri: "urn:mangala".to_string(),
            },
            unknown_tokens: vec![],
            resolution_details: vec![],
            scope: super::super::assemble::ProvenanceScope::default(),
            alternatives: vec![],
        };

        let (traverse_score, _) = score(&traverse_intent, &tokens, &report);
        let (describe_score, _) = score(&describe_intent, &tokens, &report);

        assert!(
            describe_score > traverse_score,
            "describe ({describe_score:.3}) should beat traverse ({traverse_score:.3}) with entity only"
        );
    }

    #[test]
    fn rank_sorts_best_first() {
        let tokens = vec![
            ResolvedToken::Entity {
                iri: "urn:mangala".to_string(),
                confidence: MatchConfidence::Exact,
            },
            ResolvedToken::Predicate {
                iri: "urn:exaltedIn".to_string(),
                confidence: MatchConfidence::Exact,
            },
        ];

        let attempts = vec![
            ParseAttempt {
                intent: IntentResult {
                    mode: QueryMode::Describe,
                    slot_text: "mars exalted".to_string(),
                    scope_hint: None,
                    scope_category: ScopeCategory::Unknown,
                    pattern_name: "describe_what_is",
                },
                tokens: tokens.clone(),
                report: ResolutionReport {
                    query: super::super::assemble::ResolvedQuery::Describe {
                        subject_iri: "urn:mangala".to_string(),
                    },
                    unknown_tokens: vec![],
                    resolution_details: vec![],
                    scope: super::super::assemble::ProvenanceScope::default(),
                    alternatives: vec![],
                },
            },
            ParseAttempt {
                intent: IntentResult {
                    mode: QueryMode::Traverse,
                    slot_text: "mars exalted".to_string(),
                    scope_hint: None,
                    scope_category: ScopeCategory::Unknown,
                    pattern_name: "traverse_what_is",
                },
                tokens: tokens.clone(),
                report: ResolutionReport {
                    query: super::super::assemble::ResolvedQuery::Traverse {
                        subject_iri: "urn:mangala".to_string(),
                        predicate_iri: "urn:exaltedIn".to_string(),
                    },
                    unknown_tokens: vec![],
                    resolution_details: vec![],
                    scope: super::super::assemble::ProvenanceScope::default(),
                    alternatives: vec![],
                },
            },
        ];

        let ranked = rank(attempts);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].pattern_name, "traverse_what_is");
        assert_eq!(ranked[1].pattern_name, "describe_what_is");
    }
}
