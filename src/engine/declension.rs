use serde::Deserialize;
use sqlx::PgPool;

use crate::db::ClaimRow;
use crate::error::{Result, VidyaError};
use super::{DeriveRequest, DeriveResult, EngineStrategy, TraceStep};

pub struct VyakaranaDeclensionStrategy;

impl EngineStrategy for VyakaranaDeclensionStrategy {
    fn can_handle(&self, domain: &str, operation: &str) -> bool {
        domain == "vyakarana" && operation == "declension"
    }

    fn derive<'a>(
        &'a self,
        pool: &'a PgPool,
        request: &'a DeriveRequest,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<DeriveResult>> + Send + 'a>>
    {
        Box::pin(derive_declension(pool, request))
    }

    fn analyze<'a>(
        &'a self,
        pool: &'a PgPool,
        request: &'a super::AnalyzeRequest,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<super::AnalysisCandidate>>> + Send + 'a>>
    {
        Box::pin(async move {
            let _ = (pool, request);
            Err(VidyaError::InvalidArgument {
                tool: "vidya_analyze".into(),
                argument: "operation".into(),
                constraint: "declension analysis not yet implemented".into(),
                received: "declension".into(),
            })
        })
    }
}

#[derive(Deserialize)]
struct DeclensionInput {
    stem: String,
    stem_class: String,
    vibhakti: String,
    vacana: String,
}

#[derive(Deserialize)]
struct SupSuffix {
    stem_class: String,
    vibhakti: String,
    vacana: String,
    pratyaya: String,
    suffix: String,
    markers: Vec<String>,
    sutra: String,
}

#[derive(Deserialize)]
struct PratyayaRule {
    condition_stem_class: String,
    #[serde(default)]
    condition_suffix: String,
    input_suffix: String,
    output_suffix: String,
    sutra: String,
    sutra_position: String,
    rule_type: String,
    #[serde(default)]
    condition_vibhakti: Option<String>,
}

#[derive(Deserialize)]
struct AngaRule {
    condition_stem_final: String,
    #[serde(default)]
    condition_suffix_initial: Option<String>,
    #[serde(default)]
    condition_vacana: Option<String>,
    #[allow(dead_code)]
    operation: String,
    operation_input: String,
    operation_output: String,
    sutra: String,
    sutra_position: String,
    rule_type: String,
}

#[derive(Deserialize)]
struct SandhiRule {
    first: String,
    second: String,
    result: String,
    sutra: String,
    sutra_position: String,
    rule_type: String,
}

#[derive(Deserialize)]
struct TripadiRule {
    #[serde(default)]
    condition_preceding: Option<String>,
    input: String,
    output: String,
    position: String,
    sutra: String,
    sutra_position: String,
    rule_type: String,
}

fn rule_priority(rule_type: &str) -> u8 {
    match rule_type {
        "apavada" => 3,
        "nitya" => 2,
        "utsarga" => 1,
        _ => 0,
    }
}

async fn derive_declension(pool: &PgPool, request: &DeriveRequest) -> Result<DeriveResult> {
    let input: DeclensionInput =
        serde_json::from_value(request.input.clone()).map_err(|e| VidyaError::InvalidArgument {
            tool: "vidya_derive".into(),
            argument: "input".into(),
            constraint: "requires {stem, stem_class, vibhakti, vacana}".into(),
            received: e.to_string(),
        })?;

    let mut trace = Vec::new();
    let mut step_num = 0;

    // ── Layer 1: Suffix selection ──
    let sup_claims = sqlx::query_as::<_, ClaimRow>(
        "SELECT c.* FROM claims c \
         JOIN claim_templates ct ON c.template_id = ct.id \
         WHERE c.domain_id = $1 AND ct.slug = 'sup_suffix' AND c.status = 'active' \
         ORDER BY c.created_at",
    )
    .bind(request.domain_id)
    .fetch_all(pool)
    .await?;

    let sup = sup_claims
        .iter()
        .filter_map(|c| serde_json::from_value::<SupSuffix>(c.params.clone()).ok())
        .find(|s| {
            s.stem_class == input.stem_class
                && s.vibhakti == input.vibhakti
                && s.vacana == input.vacana
        })
        .ok_or_else(|| VidyaError::InvalidArgument {
            tool: "vidya_derive".into(),
            argument: "input".into(),
            constraint: "no sup_suffix found for this combination".into(),
            received: format!(
                "{}/{}/{}",
                input.stem_class, input.vibhakti, input.vacana
            ),
        })?;

    let mut current_suffix = sup.suffix.clone();
    let pratyaya_name = sup.pratyaya.clone();
    let _markers = sup.markers.clone();
    let l1_suffix = current_suffix.clone();
    let mut pratyaya_modified = false;

    step_num += 1;
    trace.push(TraceStep {
        step: step_num,
        rule: format!("sup_suffix: {} → {}", pratyaya_name, current_suffix),
        rule_ref: Some(sup.sutra.clone()),
        input_state: format!(
            "{} + {} ({} {} {})",
            input.stem, pratyaya_name, input.stem_class, input.vibhakti, input.vacana
        ),
        output_state: format!("{} + {}", input.stem, current_suffix),
    });

    // ── Layer 2: Pratyaya modification ──
    let pratyaya_claims = sqlx::query_as::<_, ClaimRow>(
        "SELECT c.* FROM claims c \
         JOIN claim_templates ct ON c.template_id = ct.id \
         WHERE c.domain_id = $1 AND ct.slug = 'pratyaya_rule' AND c.status = 'active' \
         ORDER BY c.created_at",
    )
    .bind(request.domain_id)
    .fetch_all(pool)
    .await?;

    let mut pratyaya_rules: Vec<PratyayaRule> = pratyaya_claims
        .iter()
        .filter_map(|c| serde_json::from_value(c.params.clone()).ok())
        .collect();

    pratyaya_rules.sort_by(|a, b| {
        let pa = rule_priority(&a.rule_type);
        let pb = rule_priority(&b.rule_type);
        pb.cmp(&pa)
            .then_with(|| b.sutra_position.cmp(&a.sutra_position))
    });

    for rule in &pratyaya_rules {
        if rule.condition_stem_class != input.stem_class {
            continue;
        }
        if rule.condition_suffix != pratyaya_name {
            continue;
        }
        if let Some(ref cv) = rule.condition_vibhakti {
            if *cv != input.vibhakti {
                continue;
            }
        }
        if rule.input_suffix == current_suffix {
            let old = current_suffix.clone();
            current_suffix = rule.output_suffix.clone();
            pratyaya_modified = true;
            step_num += 1;
            trace.push(TraceStep {
                step: step_num,
                rule: format!("pratyaya_rule: {} → {}", old, current_suffix),
                rule_ref: Some(rule.sutra.clone()),
                input_state: format!("{} + {}", input.stem, old),
                output_state: format!("{} + {}", input.stem, current_suffix),
            });
            break;
        }
    }

    // ── Layer 3: Anga modification ──
    let anga_claims = sqlx::query_as::<_, ClaimRow>(
        "SELECT c.* FROM claims c \
         JOIN claim_templates ct ON c.template_id = ct.id \
         WHERE c.domain_id = $1 AND ct.slug = 'anga_rule' AND c.status = 'active' \
         ORDER BY c.created_at",
    )
    .bind(request.domain_id)
    .fetch_all(pool)
    .await?;

    let mut anga_rules: Vec<AngaRule> = anga_claims
        .iter()
        .filter_map(|c| serde_json::from_value(c.params.clone()).ok())
        .collect();

    anga_rules.sort_by(|a, b| {
        let pa = rule_priority(&a.rule_type);
        let pb = rule_priority(&b.rule_type);
        pb.cmp(&pa)
            .then_with(|| b.sutra_position.cmp(&a.sutra_position))
    });

    let stem_final = input.stem.chars().last().map(|c| c.to_string()).unwrap_or_default();
    let suffix_initial = first_phoneme(&current_suffix);
    let mut current_stem = input.stem.clone();

    for rule in &anga_rules {
        if rule.condition_stem_final != stem_final {
            continue;
        }
        if let Some(ref si) = rule.condition_suffix_initial {
            if *si != suffix_initial {
                continue;
            }
        }
        if let Some(ref cv) = rule.condition_vacana {
            if *cv != input.vacana {
                continue;
            }
        }
        if current_stem.ends_with(&rule.operation_input) {
            let old_stem = current_stem.clone();
            let prefix = &current_stem[..current_stem.len() - rule.operation_input.len()];
            current_stem = format!("{}{}", prefix, rule.operation_output);
            step_num += 1;
            trace.push(TraceStep {
                step: step_num,
                rule: format!(
                    "anga_rule: {} → {} (stem: {} → {})",
                    rule.operation_input, rule.operation_output, old_stem, current_stem
                ),
                rule_ref: Some(rule.sutra.clone()),
                input_state: format!("{} + {}", old_stem, current_suffix),
                output_state: format!("{} + {}", current_stem, current_suffix),
            });
            break;
        }
    }

    // ── Layer 4: Junction sandhi ──
    if !current_suffix.is_empty() {
        let sandhi_claims = sqlx::query_as::<_, ClaimRow>(
            "SELECT c.* FROM claims c \
             JOIN claim_templates ct ON c.template_id = ct.id \
             WHERE c.domain_id = $1 AND ct.slug = 'sandhi_rule' AND c.status = 'active' \
             ORDER BY c.created_at",
        )
        .bind(request.domain_id)
        .fetch_all(pool)
        .await?;

        let mut sandhi_rules: Vec<SandhiRule> = sandhi_claims
            .iter()
            .filter_map(|c| serde_json::from_value(c.params.clone()).ok())
            .collect();

        sandhi_rules.sort_by(|a, b| {
            let pa = rule_priority(&a.rule_type);
            let pb = rule_priority(&b.rule_type);
            pb.cmp(&pa)
                .then_with(|| b.sutra_position.cmp(&a.sutra_position))
        });

        let stem_end = last_phoneme(&current_stem);
        let suf_start = first_phoneme(&current_suffix);

        // Sthānivat: skip savarṇa-dīrgha (a+a→ā etc.) when the suffix
        // vowel at the junction is "inherited" — same vowel as stem-final,
        // not introduced by anubandha stripping or pratyaya modification.
        // When pratyaya "jas" → suffix "as", the initial vowel changed
        // (j stripped), so the suffix 'a' is genuinely new at the boundary.
        // When pratyaya "am" → suffix "am" (unchanged), the 'a' is the
        // stem vowel continuing — no savarṇa meeting.
        let suffix_vowel_is_new = pratyaya_modified
            || first_phoneme(&pratyaya_name) != first_phoneme(&l1_suffix);

        // When the suffix vowel is inherited (not new), absorb the
        // duplicate at the junction: deva + am → devam (not devaam).
        // The stem-final vowel subsumes the suffix-initial.
        if !suffix_vowel_is_new && is_savarna(&stem_end, &suf_start) {
            let remainder = strip_first_phoneme(&current_suffix, &suf_start);
            current_suffix = remainder;
        }

        let stem_end = last_phoneme(&current_stem);
        let suf_start = first_phoneme(&current_suffix);

        for rule in &sandhi_rules {
            if rule.first == stem_end && rule.second == suf_start {
                let old_stem = current_stem.clone();
                let old_suffix = current_suffix.clone();

                let prefix = strip_last_phoneme(&current_stem, &stem_end);
                let remainder = strip_first_phoneme(&current_suffix, &suf_start);
                let combined = format!("{}{}{}", prefix, rule.result, remainder);

                step_num += 1;
                trace.push(TraceStep {
                    step: step_num,
                    rule: format!(
                        "sandhi: {} + {} → {}",
                        rule.first, rule.second, rule.result
                    ),
                    rule_ref: Some(rule.sutra.clone()),
                    input_state: format!("{} + {}", old_stem, old_suffix),
                    output_state: combined.clone(),
                });

                current_stem = combined;
                current_suffix = String::new();
                break;
            }
        }
    }

    let mut result = if current_suffix.is_empty() {
        current_stem
    } else {
        format!("{}{}", current_stem, current_suffix)
    };

    // ── Layer 5: Tripadi ──
    let tripadi_claims = sqlx::query_as::<_, ClaimRow>(
        "SELECT c.* FROM claims c \
         JOIN claim_templates ct ON c.template_id = ct.id \
         WHERE c.domain_id = $1 AND ct.slug = 'tripadi_rule' AND c.status = 'active' \
         ORDER BY c.created_at",
    )
    .bind(request.domain_id)
    .fetch_all(pool)
    .await?;

    let mut tripadi_rules: Vec<TripadiRule> = tripadi_claims
        .iter()
        .filter_map(|c| serde_json::from_value(c.params.clone()).ok())
        .collect();

    tripadi_rules.sort_by(|a, b| {
        let pa = rule_priority(&a.rule_type);
        let pb = rule_priority(&b.rule_type);
        pb.cmp(&pa)
            .then_with(|| b.sutra_position.cmp(&a.sutra_position))
    });

    for rule in &tripadi_rules {
        let applied = match rule.position.as_str() {
            "word_final" => {
                if result.ends_with(&rule.input) {
                    let prefix = &result[..result.len() - rule.input.len()];
                    Some(format!("{}{}", prefix, rule.output))
                } else {
                    None
                }
            }
            "internal" => {
                if let Some(ref prec) = rule.condition_preceding {
                    if prec == "iuk" {
                        try_apply_iuk_retroflexion(&result, &rule.input, &rule.output)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(new_result) = applied {
            let old = result.clone();
            result = new_result;
            step_num += 1;
            trace.push(TraceStep {
                step: step_num,
                rule: format!("tripadi: {} → {}", rule.input, rule.output),
                rule_ref: Some(rule.sutra.clone()),
                input_state: old,
                output_state: result.clone(),
            });
            break;
        }
    }

    Ok(DeriveResult {
        output: serde_json::json!({
            "stem": input.stem,
            "vibhakti": input.vibhakti,
            "vacana": input.vacana,
            "form": result,
            "steps": trace.len(),
        }),
        trace,
    })
}

fn first_phoneme(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if first == 'b' {
        if let Some('h') = chars.next() {
            return "bh".to_string();
        }
    }
    if first == 'a' {
        if let Some(c) = chars.next() {
            if c == 'i' || c == 'u' {
                return format!("a{}", c);
            }
        }
    }
    first.to_string()
}

fn last_phoneme(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    let last = *chars.last().unwrap();
    if chars.len() >= 2 {
        let penult = chars[chars.len() - 2];
        if penult == 'a' && (last == 'i' || last == 'u') {
            return format!("a{}", last);
        }
    }
    last.to_string()
}

fn strip_last_phoneme(s: &str, phoneme: &str) -> String {
    if s.ends_with(phoneme) {
        s[..s.len() - phoneme.len()].to_string()
    } else {
        s.to_string()
    }
}

fn strip_first_phoneme(s: &str, phoneme: &str) -> String {
    if s.starts_with(phoneme) {
        s[phoneme.len()..].to_string()
    } else {
        s.to_string()
    }
}

const IUK_VOWELS: &[char] = &['i', 'u', 'e', 'o'];

fn base_vowel(phoneme: &str) -> &str {
    match phoneme {
        "a" | "ā" => "a",
        "i" | "ī" => "i",
        "u" | "ū" => "u",
        "ṛ" | "ṝ" => "ṛ",
        "ḷ" => "ḷ",
        _ => phoneme,
    }
}

fn is_savarna(a: &str, b: &str) -> bool {
    base_vowel(a) == base_vowel(b)
}

fn try_apply_iuk_retroflexion(word: &str, input: &str, output: &str) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        // Only internal positions — word-final is handled by 8.2.66
        if ch.to_string() == input && i > 0 && i < chars.len() - 1 {
            let preceding = chars[i - 1];
            if IUK_VOWELS.contains(&preceding) || preceding == 'ṛ' {
                let mut new_word: String = chars[..i].iter().collect();
                new_word.push_str(output);
                let rest: String = chars[i + 1..].iter().collect();
                new_word.push_str(&rest);
                return Some(new_word);
            }
        }
    }
    None
}
