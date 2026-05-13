use serde::Deserialize;
use sqlx::PgPool;

use crate::db::ClaimRow;
use crate::error::{Result, VidyaError};
use super::{AnalyzeRequest, AnalysisCandidate, DeriveRequest, DeriveResult, EngineStrategy, TraceStep};
use super::phoneme::{phoneme_ends_with, phoneme_starts_with, phoneme_strip_suffix, phoneme_strip_prefix};

pub struct VyakaranaSandhiStrategy;

impl EngineStrategy for VyakaranaSandhiStrategy {
    fn can_handle(&self, domain: &str, operation: &str) -> bool {
        domain == "vyakarana" && operation == "sandhi"
    }

    fn derive<'a>(
        &'a self,
        pool: &'a PgPool,
        request: &'a DeriveRequest,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<DeriveResult>> + Send + 'a>> {
        Box::pin(derive_sandhi(pool, request))
    }

    fn analyze<'a>(
        &'a self,
        pool: &'a PgPool,
        request: &'a AnalyzeRequest,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<AnalysisCandidate>>> + Send + 'a>> {
        Box::pin(analyze_sandhi(pool, request))
    }
}

#[derive(Debug, Deserialize)]
struct SandhiInput {
    first: String,
    second: String,
}

#[derive(Debug, Deserialize)]
struct AnalyzeInput {
    form: String,
}

#[derive(Debug, Deserialize)]
struct SandhiParams {
    first: String,
    second: String,
    result: String,
    #[serde(default)]
    sutra: String,
    #[serde(default)]
    sutra_position: String,
    #[serde(default)]
    rule_type: String,
}

fn rule_type_priority(rule_type: &str) -> u8 {
    match rule_type {
        "apavāda" | "apavada" => 4,
        "nitya" => 3,
        "paribhāṣā" | "paribhasha" => 2,
        "utsarga" => 1,
        _ => 0,
    }
}

async fn derive_sandhi(pool: &PgPool, request: &DeriveRequest) -> Result<DeriveResult> {
    let input: SandhiInput =
        serde_json::from_value(request.input.clone()).map_err(|e| VidyaError::InvalidArgument {
            tool: "vidya_derive".into(),
            argument: "input".into(),
            constraint: "requires {first, second} fields".into(),
            received: e.to_string(),
        })?;

    let rules = sqlx::query_as::<_, ClaimRow>(
        "SELECT c.* FROM claims c \
         JOIN claim_templates ct ON c.template_id = ct.id \
         WHERE c.domain_id = $1 AND ct.slug = 'sandhi_rule' AND c.status = 'active' \
         ORDER BY c.created_at",
    )
    .bind(request.domain_id)
    .fetch_all(pool)
    .await?;

    let mut parsed_rules: Vec<(SandhiParams, &ClaimRow)> = rules
        .iter()
        .filter_map(|rule| {
            serde_json::from_value::<SandhiParams>(rule.params.clone())
                .ok()
                .map(|p| (p, rule))
        })
        .collect();

    // Sort by conflict resolution: apavāda > nitya > utsarga, then by sutra_position (later wins)
    parsed_rules.sort_by(|(a, _), (b, _)| {
        let pa = rule_type_priority(&a.rule_type);
        let pb = rule_type_priority(&b.rule_type);
        pb.cmp(&pa).then_with(|| b.sutra_position.cmp(&a.sutra_position))
    });

    let mut trace = Vec::new();
    let mut current_first = input.first.clone();
    let mut current_second = input.second.clone();
    let mut result_str = format!("{}{}", current_first, current_second);

    for iteration in 0..100 {
        let mut matched = false;

        for (params, rule) in &parsed_rules {
            if phoneme_ends_with(&current_first, &params.first) && phoneme_starts_with(&current_second, &params.second) {
                let input_state = format!("{} + {}", current_first, current_second);

                let prefix = phoneme_strip_suffix(&current_first, &params.first).unwrap();
                let suffix = phoneme_strip_prefix(&current_second, &params.second).unwrap();
                result_str = format!("{}{}{}", prefix, params.result, suffix);

                trace.push(TraceStep {
                    step: iteration + 1,
                    rule: rule.statement.clone(),
                    rule_ref: if params.sutra.is_empty() {
                        None
                    } else {
                        Some(params.sutra.clone())
                    },
                    input_state,
                    output_state: result_str.clone(),
                });

                current_first = result_str.clone();
                current_second = String::new();
                matched = true;
                break;
            }
        }

        if !matched || current_second.is_empty() {
            break;
        }
    }

    Ok(DeriveResult {
        output: serde_json::json!({
            "input": format!("{} + {}", input.first, input.second),
            "result": result_str,
            "steps": trace.len(),
        }),
        trace,
    })
}

async fn analyze_sandhi(pool: &PgPool, request: &AnalyzeRequest) -> Result<Vec<AnalysisCandidate>> {
    let input: AnalyzeInput =
        serde_json::from_value(request.input.clone()).map_err(|e| VidyaError::InvalidArgument {
            tool: "vidya_analyze".into(),
            argument: "input".into(),
            constraint: "requires {form} field".into(),
            received: e.to_string(),
        })?;

    let rules = sqlx::query_as::<_, ClaimRow>(
        "SELECT c.* FROM claims c \
         JOIN claim_templates ct ON c.template_id = ct.id \
         WHERE c.domain_id = $1 AND ct.slug = 'sandhi_rule' AND c.status = 'active' \
         ORDER BY c.created_at",
    )
    .bind(request.domain_id)
    .fetch_all(pool)
    .await?;

    let parsed_rules: Vec<(SandhiParams, &ClaimRow)> = rules
        .iter()
        .filter_map(|rule| {
            serde_json::from_value::<SandhiParams>(rule.params.clone())
                .ok()
                .map(|p| (p, rule))
        })
        .collect();

    let form_phonemes = super::phoneme::tokenize(&input.form);
    let mut candidates = Vec::new();

    for (params, rule) in &parsed_rules {
        let result_phonemes = super::phoneme::tokenize(&params.result);
        if result_phonemes.is_empty() {
            continue;
        }

        // Find every position where rule.result appears as a contiguous subsequence
        let rlen = result_phonemes.len();
        let flen = form_phonemes.len();
        if rlen > flen {
            continue;
        }

        for start in 0..=(flen - rlen) {
            if form_phonemes[start..start + rlen] == result_phonemes[..] {
                let prefix: String = form_phonemes[..start].concat();
                let suffix: String = form_phonemes[start + rlen..].concat();

                let first = format!("{}{}", prefix, params.first);
                let second = format!("{}{}", params.second, suffix);

                candidates.push(AnalysisCandidate {
                    decomposition: serde_json::json!({
                        "first": first,
                        "second": second,
                    }),
                    rule: rule.statement.clone(),
                    rule_ref: if params.sutra.is_empty() {
                        None
                    } else {
                        Some(params.sutra.clone())
                    },
                    specificity: rule_type_priority(&params.rule_type) as f64,
                });
            }
        }
    }

    // Sort by specificity descending (higher = more specific rule)
    candidates.sort_by(|a, b| b.specificity.partial_cmp(&a.specificity).unwrap());

    Ok(candidates)
}
