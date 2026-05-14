use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

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
        Box::pin(analyze_declension(pool, request))
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
struct AnalyzeDeclensionInput {
    form: String,
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
    #[serde(default)]
    condition_pratyaya: Option<String>,
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

struct DeclensionRuleSets {
    sup_suffixes: Vec<SupSuffix>,
    pratyaya_rules: Vec<PratyayaRule>,
    anga_rules: Vec<AngaRule>,
    sandhi_rules: Vec<SandhiRule>,
    tripadi_rules: Vec<TripadiRule>,
}

fn parse_claims<T: for<'de> Deserialize<'de>>(claims: &[ClaimRow], label: &str) -> Vec<T> {
    claims
        .iter()
        .filter_map(|c| {
            serde_json::from_value(c.params.clone()).map_err(|e| {
                tracing::warn!(claim_id = %c.id, template = label, error = %e, "claim deserialization failed, skipping");
                e
            }).ok()
        })
        .collect()
}

fn sort_by_priority<T>(rules: &mut [T], get_rule_type: impl Fn(&T) -> &str, get_position: impl Fn(&T) -> &str) {
    rules.sort_by(|a, b| {
        rule_priority(get_rule_type(b))
            .cmp(&rule_priority(get_rule_type(a)))
            .then_with(|| get_position(b).cmp(get_position(a)))
    });
}

async fn load_rule_sets(pool: &PgPool, domain_id: Uuid) -> Result<DeclensionRuleSets> {
    let query = |slug: &'static str| {
        sqlx::query_as::<_, ClaimRow>(
            "SELECT c.* FROM claims c \
             JOIN claim_templates ct ON c.template_id = ct.id \
             WHERE c.domain_id = $1 AND ct.slug = $2 AND c.status = 'active' \
             ORDER BY c.created_at",
        )
        .bind(domain_id)
        .bind(slug)
        .fetch_all(pool)
    };

    let (sup_claims, pratyaya_claims, anga_claims, sandhi_claims, tripadi_claims) = tokio::try_join!(
        query("sup_suffix"),
        query("pratyaya_rule"),
        query("anga_rule"),
        query("sandhi_rule"),
        query("tripadi_rule"),
    )?;

    let sup_suffixes = parse_claims(&sup_claims, "sup_suffix");
    let mut pratyaya_rules: Vec<PratyayaRule> = parse_claims(&pratyaya_claims, "pratyaya_rule");
    let mut anga_rules: Vec<AngaRule> = parse_claims(&anga_claims, "anga_rule");
    let mut sandhi_rules: Vec<SandhiRule> = parse_claims(&sandhi_claims, "sandhi_rule");
    let mut tripadi_rules: Vec<TripadiRule> = parse_claims(&tripadi_claims, "tripadi_rule");

    sort_by_priority(&mut pratyaya_rules, |r| &r.rule_type, |r| &r.sutra_position);
    sort_by_priority(&mut anga_rules, |r| &r.rule_type, |r| &r.sutra_position);
    sort_by_priority(&mut sandhi_rules, |r| &r.rule_type, |r| &r.sutra_position);
    // Tripadi: ascending sutra_position (rules apply in forward order within 8.2-8.4)
    tripadi_rules.sort_by(|a, b| {
        rule_priority(&b.rule_type)
            .cmp(&rule_priority(&a.rule_type))
            .then_with(|| a.sutra_position.cmp(&b.sutra_position))
    });

    Ok(DeclensionRuleSets { sup_suffixes, pratyaya_rules, anga_rules, sandhi_rules, tripadi_rules })
}

async fn derive_declension(pool: &PgPool, request: &DeriveRequest) -> Result<DeriveResult> {
    let input: DeclensionInput =
        serde_json::from_value(request.input.clone()).map_err(|e| VidyaError::InvalidArgument {
            tool: "vidya_derive".into(),
            argument: "input".into(),
            constraint: "requires {stem, stem_class, vibhakti, vacana}".into(),
            received: e.to_string(),
        })?;

    let rules = load_rule_sets(pool, request.domain_id).await?;

    let mut trace = Vec::new();
    let mut step_num = 0;

    // ── Layer 1: Suffix selection ──
    let sup = rules.sup_suffixes
        .iter()
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
    for rule in &rules.pratyaya_rules {
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
    let stem_final = input.stem.chars().last().map(|c| c.to_string()).unwrap_or_default();
    let suffix_initial = first_phoneme(&current_suffix);
    let mut current_stem = input.stem.clone();

    for rule in &rules.anga_rules {
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
        let stem_end = last_phoneme(&current_stem);
        let suf_start = first_phoneme(&current_suffix);

        for rule in &rules.sandhi_rules {
            if let Some(ref required) = rule.condition_pratyaya {
                if *required != pratyaya_name {
                    continue;
                }
            }
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
    for rule in &rules.tripadi_rules {
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

// Assumes stem class naming: "{stem-final-vowel}-stem-{gender}" (e.g. "a-stem-m")
fn stem_final_for_class(stem_class: &str) -> String {
    stem_class.split('-').next().unwrap_or("a").to_string()
}

async fn analyze_declension(
    pool: &PgPool,
    request: &super::AnalyzeRequest,
) -> Result<Vec<super::AnalysisCandidate>> {
    let input: AnalyzeDeclensionInput =
        serde_json::from_value(request.input.clone()).map_err(|e| VidyaError::InvalidArgument {
            tool: "vidya_analyze".into(),
            argument: "input".into(),
            constraint: "requires {form} field".into(),
            received: e.to_string(),
        })?;
    let form = &input.form;

    let rules = load_rule_sets(pool, request.domain_id).await?;

    let mut candidates = Vec::new();

    for sup in &rules.sup_suffixes {
        if let Some(candidate) = try_match_sup(
            form,
            sup,
            &rules.pratyaya_rules,
            &rules.anga_rules,
            &rules.sandhi_rules,
            &rules.tripadi_rules,
        ) {
            candidates.push(candidate);
        }
    }

    candidates.sort_by(|a, b| {
        b.specificity
            .partial_cmp(&a.specificity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(candidates)
}

fn try_match_sup(
    form: &str,
    sup: &SupSuffix,
    pratyaya_rules: &[PratyayaRule],
    anga_rules: &[AngaRule],
    sandhi_rules: &[SandhiRule],
    tripadi_rules: &[TripadiRule],
) -> Option<super::AnalysisCandidate> {
    let stem_final = stem_final_for_class(&sup.stem_class);
    let mut trace = Vec::new();
    let mut step_num = 0usize;

    step_num += 1;
    trace.push(json!({
        "step": step_num,
        "rule": format!("sup_suffix: {} → {}", sup.pratyaya, sup.suffix),
        "sutra": sup.sutra,
    }));

    // Layer 2: Pratyaya modification
    let mut current_suffix = sup.suffix.clone();
    for rule in pratyaya_rules {
        if rule.condition_stem_class != sup.stem_class {
            continue;
        }
        if rule.condition_suffix != sup.pratyaya {
            continue;
        }
        if let Some(ref cv) = rule.condition_vibhakti {
            if *cv != sup.vibhakti {
                continue;
            }
        }
        if rule.input_suffix == current_suffix {
            let old = current_suffix.clone();
            current_suffix = rule.output_suffix.clone();
            step_num += 1;
            trace.push(json!({
                "step": step_num,
                "rule": format!("pratyaya_rule: {} → {}", old, current_suffix),
                "sutra": rule.sutra,
            }));
            break;
        }
    }

    // Layer 3: Anga modification
    let suffix_initial = first_phoneme(&current_suffix);
    let mut modified_final = stem_final.clone();
    for rule in anga_rules {
        if rule.condition_stem_final != stem_final {
            continue;
        }
        if let Some(ref si) = rule.condition_suffix_initial {
            if *si != suffix_initial {
                continue;
            }
        }
        if let Some(ref cv) = rule.condition_vacana {
            if *cv != sup.vacana {
                continue;
            }
        }
        if modified_final == rule.operation_input {
            let old = modified_final.clone();
            modified_final = rule.operation_output.clone();
            step_num += 1;
            trace.push(json!({
                "step": step_num,
                "rule": format!("anga_rule: {} → {}", old, modified_final),
                "sutra": rule.sutra,
            }));
            break;
        }
    }

    // Layer 4: Junction sandhi
    let tail = if current_suffix.is_empty() {
        modified_final.clone()
    } else {
        let stem_end = &modified_final;
        let suf_start = first_phoneme(&current_suffix);
        let mut tail_result = format!("{}{}", modified_final, current_suffix);

        for rule in sandhi_rules {
            if let Some(ref required) = rule.condition_pratyaya {
                if *required != sup.pratyaya {
                    continue;
                }
            }
            if rule.first == *stem_end && rule.second == suf_start {
                let remainder = strip_first_phoneme(&current_suffix, &suf_start);
                tail_result = format!("{}{}", rule.result, remainder);
                step_num += 1;
                trace.push(json!({
                    "step": step_num,
                    "rule": format!("sandhi: {} + {} → {}", rule.first, rule.second, rule.result),
                    "sutra": rule.sutra,
                }));
                break;
            }
        }
        tail_result
    };

    // Layer 5: Tripadi
    let mut final_tail = tail;
    for rule in tripadi_rules {
        let applied = match rule.position.as_str() {
            "word_final" => {
                if final_tail.ends_with(&rule.input) {
                    let prefix = &final_tail[..final_tail.len() - rule.input.len()];
                    Some(format!("{}{}", prefix, rule.output))
                } else {
                    None
                }
            }
            "internal" => {
                if let Some(ref prec) = rule.condition_preceding {
                    if prec == "iuk" {
                        try_apply_iuk_retroflexion(&final_tail, &rule.input, &rule.output)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(new_tail) = applied {
            step_num += 1;
            trace.push(json!({
                "step": step_num,
                "rule": format!("tripadi: {} → {}", rule.input, rule.output),
                "sutra": rule.sutra,
            }));
            final_tail = new_tail;
        }
    }

    // Match against form
    if final_tail.is_empty() || form.len() < final_tail.len() {
        return None;
    }
    if !form.ends_with(&final_tail) {
        return None;
    }

    let prefix = &form[..form.len() - final_tail.len()];
    let stem = format!("{}{}", prefix, stem_final);

    Some(super::AnalysisCandidate {
        decomposition: json!({
            "stem": stem,
            "stem_class": sup.stem_class,
            "vibhakti": sup.vibhakti,
            "vacana": sup.vacana,
            "trace": trace,
        }),
        rule: format!("{} {} {}", sup.stem_class, sup.vibhakti, sup.vacana),
        rule_ref: Some(sup.sutra.clone()),
        specificity: final_tail.chars().count() as f64,
    })
}
