use serde::Deserialize;
use sqlx::PgPool;

use crate::db::ClaimRow;
use crate::error::{Result, VidyaError};
use super::{DeriveRequest, DeriveResult, TraceStep};

#[derive(Debug, Deserialize)]
struct SandhiInput {
    first: String,
    second: String,
}

#[derive(Debug, Deserialize)]
struct SandhiParams {
    first: String,
    second: String,
    result: String,
    #[serde(default)]
    sutra: String,
}

pub async fn derive_sandhi(pool: &PgPool, request: &DeriveRequest) -> Result<DeriveResult> {
    let input: SandhiInput =
        serde_json::from_value(request.input.clone()).map_err(|e| VidyaError::InvalidArgument {
            tool: "vidya_derive".into(),
            argument: "input".into(),
            constraint: "requires {first, second} fields".into(),
            received: e.to_string(),
        })?;

    // Load all active sandhi_rule claims for this domain
    let rules = sqlx::query_as::<_, ClaimRow>(
        "SELECT c.* FROM claims c \
         JOIN claim_templates ct ON c.template_id = ct.id \
         WHERE c.domain_id = $1 AND ct.slug = 'sandhi_rule' AND c.status = 'active' \
         ORDER BY c.created_at",
    )
    .bind(request.domain_id)
    .fetch_all(pool)
    .await?;

    let mut trace = Vec::new();
    let mut current_first = input.first.clone();
    let mut current_second = input.second.clone();
    let mut result_str = format!("{}{}", current_first, current_second);
    let max_iterations = 100;

    for iteration in 0..max_iterations {
        let mut matched = false;

        for rule in &rules {
            let params: SandhiParams = match serde_json::from_value(rule.params.clone()) {
                Ok(p) => p,
                Err(_) => continue,
            };

            // Check if this rule matches: the end of first matches params.first
            // and the start of second matches params.second
            if current_first.ends_with(&params.first) && current_second.starts_with(&params.second)
            {
                let input_state = format!("{} + {}", current_first, current_second);

                // Apply the rule: replace the junction
                let prefix = &current_first[..current_first.len() - params.first.len()];
                let suffix = &current_second[params.second.len()..];
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

                // For subsequent iterations, treat result as a single unit
                current_first = result_str.clone();
                current_second = String::new();
                matched = true;
                break;
            }
        }

        if !matched {
            break;
        }

        if current_second.is_empty() {
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
