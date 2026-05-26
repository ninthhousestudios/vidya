use super::assemble::QueryMode;

#[derive(Debug)]
pub struct IntentResult {
    pub mode: QueryMode,
    pub slot_text: String,
    pub scope_hint: Option<String>,
    pub pattern_name: &'static str,
}

pub fn detect_intent(raw_input: &str) -> Option<IntentResult> {
    let n = normalize(raw_input);
    let n = n.as_str();

    if let Some(r) = try_tradition_say(n) {
        return Some(r);
    }
    if let Some(r) = try_tradition_according(n) {
        return Some(r);
    }
    if let Some(r) = try_pramana_from(n) {
        return Some(r);
    }
    if let Some(r) = try_similar(n) {
        return Some(r);
    }
    if let Some(r) = try_search_where(n) {
        return Some(r);
    }
    if let Some(r) = try_search_what_are(n) {
        return Some(r);
    }
    if let Some(r) = try_traverse_possessive(n) {
        return Some(r);
    }
    if let Some(r) = try_traverse_does(n) {
        return Some(r);
    }
    if let Some(r) = try_describe_tell(n) {
        return Some(r);
    }
    if let Some(r) = try_describe_explicit(n) {
        return Some(r);
    }
    if let Some(r) = try_traverse_what_is(n) {
        return Some(r);
    }
    if let Some(r) = try_describe_what_is(n) {
        return Some(r);
    }

    None
}

pub fn detect_all_intents(raw_input: &str) -> Vec<IntentResult> {
    let n = normalize(raw_input);
    let n = n.as_str();

    let mut results = Vec::new();
    if let Some(r) = try_tradition_say(n) { results.push(r); }
    if let Some(r) = try_tradition_according(n) { results.push(r); }
    if let Some(r) = try_pramana_from(n) { results.push(r); }
    if let Some(r) = try_similar(n) { results.push(r); }
    if let Some(r) = try_search_where(n) { results.push(r); }
    if let Some(r) = try_search_what_are(n) { results.push(r); }
    if let Some(r) = try_traverse_possessive(n) { results.push(r); }
    if let Some(r) = try_traverse_does(n) { results.push(r); }
    if let Some(r) = try_traverse_what_is(n) { results.push(r); }
    if let Some(r) = try_describe_tell(n) { results.push(r); }
    if let Some(r) = try_describe_explicit(n) { results.push(r); }
    if let Some(r) = try_describe_what_is(n) { results.push(r); }
    results
}

fn normalize(input: &str) -> String {
    let s: String = input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    s.trim_end_matches(|c: char| matches!(c, '?' | '!' | '.'))
        .trim()
        .to_string()
}

fn non_empty(s: &str) -> Option<&str> {
    let s = s.trim();
    if s.is_empty() { None } else { Some(s) }
}

// "what does T say about X"
fn try_tradition_say(input: &str) -> Option<IntentResult> {
    let rest = input.strip_prefix("what does ")?;
    let idx = rest.find(" say about ")?;
    let tradition = non_empty(&rest[..idx])?;
    let remainder = non_empty(&rest[idx + " say about ".len()..])?;

    let inner = detect_intent(remainder).unwrap_or(IntentResult {
        mode: QueryMode::Describe,
        slot_text: remainder.to_string(),
        scope_hint: None,
        pattern_name: "describe_fallback",
    });

    Some(IntentResult {
        scope_hint: Some(tradition.to_string()),
        pattern_name: "tradition_say",
        ..inner
    })
}

// "according to T, X" / "in T, X"
fn try_tradition_according(input: &str) -> Option<IntentResult> {
    let (tradition_text, rest) = if let Some(rest) = input.strip_prefix("according to ") {
        let idx = rest.find(',')?;
        (non_empty(&rest[..idx])?, non_empty(&rest[idx + 1..])?)
    } else if let Some(rest) = input.strip_prefix("in ") {
        let idx = rest.find(',')?;
        (non_empty(&rest[..idx])?, non_empty(&rest[idx + 1..])?)
    } else {
        return None;
    };

    let inner = detect_intent(rest).unwrap_or(IntentResult {
        mode: QueryMode::Describe,
        slot_text: rest.to_string(),
        scope_hint: None,
        pattern_name: "describe_fallback",
    });

    Some(IntentResult {
        scope_hint: Some(tradition_text.to_string()),
        pattern_name: "tradition_according",
        ..inner
    })
}

// "show claims from X pramana about Y", "claims from X about Y", "from X pramana, Y"
fn try_pramana_from(input: &str) -> Option<IntentResult> {
    let rest = if let Some(r) = input.strip_prefix("show claims from ") {
        r
    } else if let Some(r) = input.strip_prefix("claims from ") {
        r
    } else if let Some(r) = input.strip_prefix("from ") {
        r
    } else {
        return None;
    };

    // Try "X pramana about Y" first
    if let Some(idx) = rest.find(" pramana about ") {
        let pramana = non_empty(&rest[..idx])?;
        let remainder = non_empty(&rest[idx + " pramana about ".len()..])?;
        let inner = detect_intent(remainder).unwrap_or(IntentResult {
            mode: QueryMode::Describe,
            slot_text: remainder.to_string(),
            scope_hint: None,
            pattern_name: "describe_fallback",
        });
        return Some(IntentResult {
            scope_hint: Some(pramana.to_string()),
            pattern_name: "pramana_from",
            ..inner
        });
    }

    // Try "X pramana, Y"
    if let Some(idx) = rest.find(" pramana, ") {
        let pramana = non_empty(&rest[..idx])?;
        let remainder = non_empty(&rest[idx + " pramana, ".len()..])?;
        let inner = detect_intent(remainder).unwrap_or(IntentResult {
            mode: QueryMode::Describe,
            slot_text: remainder.to_string(),
            scope_hint: None,
            pattern_name: "describe_fallback",
        });
        return Some(IntentResult {
            scope_hint: Some(pramana.to_string()),
            pattern_name: "pramana_from",
            ..inner
        });
    }

    // Try "X about Y" (no "pramana" keyword)
    if let Some(idx) = rest.find(" about ") {
        let scope_name = non_empty(&rest[..idx])?;
        let remainder = non_empty(&rest[idx + " about ".len()..])?;
        let inner = detect_intent(remainder).unwrap_or(IntentResult {
            mode: QueryMode::Describe,
            slot_text: remainder.to_string(),
            scope_hint: None,
            pattern_name: "describe_fallback",
        });
        return Some(IntentResult {
            scope_hint: Some(scope_name.to_string()),
            pattern_name: "pramana_from",
            ..inner
        });
    }

    None
}

// "similar to X", "what is related to X", "things like X", "entities like X"
fn try_similar(input: &str) -> Option<IntentResult> {
    let slot = if let Some(rest) = input.strip_prefix("similar to ") {
        non_empty(rest)?
    } else if let Some(idx) = input.find("related to ") {
        non_empty(&input[idx + "related to ".len()..])?
    } else if let Some(rest) = input.strip_prefix("things like ") {
        non_empty(rest)?
    } else if let Some(rest) = input.strip_prefix("entities like ") {
        non_empty(rest)?
    } else {
        return None;
    };

    Some(IntentResult {
        mode: QueryMode::Similar,
        slot_text: slot.to_string(),
        scope_hint: None,
        pattern_name: "similar",
    })
}

// "find Xs where Y is Z"
fn try_search_where(input: &str) -> Option<IntentResult> {
    let rest = input.strip_prefix("find ")?;
    let where_idx = rest.find(" where ")?;
    let type_text = non_empty(&rest[..where_idx])?;
    let filter_text = non_empty(&rest[where_idx + " where ".len()..])?;

    // Combine type + filter text for the existing pipeline
    // The "is" in "Y is Z" will be stripped as a stopword
    let slot = format!("{type_text} {filter_text}");
    Some(IntentResult {
        mode: QueryMode::Search,
        slot_text: slot,
        scope_hint: None,
        pattern_name: "search_where",
    })
}

// "what Xs are Y" — the "are" distinguishes from "what is X"
fn try_search_what_are(input: &str) -> Option<IntentResult> {
    let rest = input.strip_prefix("what ")?;
    let idx = rest.find(" are ")?;
    let type_text = non_empty(&rest[..idx])?;
    let filter_text = non_empty(&rest[idx + " are ".len()..])?;

    let slot = format!("{type_text} {filter_text}");
    Some(IntentResult {
        mode: QueryMode::Search,
        slot_text: slot,
        scope_hint: None,
        pattern_name: "search_what_are",
    })
}

// "what is X's Y"
fn try_traverse_possessive(input: &str) -> Option<IntentResult> {
    let rest = input.strip_prefix("what is ")?;
    // Look for 's (apostrophe-s)
    let idx = rest.find("'s ")?;
    let subject = non_empty(&rest[..idx])?;
    let predicate = non_empty(&rest[idx + "'s ".len()..])?;

    let slot = format!("{subject} {predicate}");
    Some(IntentResult {
        mode: QueryMode::Traverse,
        slot_text: slot,
        scope_hint: None,
        pattern_name: "traverse_possessive",
    })
}

// "what does X Y" (with at least two words after "what does")
fn try_traverse_does(input: &str) -> Option<IntentResult> {
    let rest = input.strip_prefix("what does ")?;
    let rest = non_empty(rest)?;
    // Need at least two tokens (subject + predicate)
    if !rest.contains(' ') {
        return None;
    }
    Some(IntentResult {
        mode: QueryMode::Traverse,
        slot_text: rest.to_string(),
        scope_hint: None,
        pattern_name: "traverse_does",
    })
}

// "what is X Y" where there are >= 2 content words — candidate traverse
fn try_traverse_what_is(input: &str) -> Option<IntentResult> {
    let rest = input.strip_prefix("what is ")?;
    let rest = non_empty(rest)?;
    let content_words: Vec<&str> = rest
        .split_whitespace()
        .filter(|w| !super::matcher::STOPWORDS.contains(w))
        .collect();
    if content_words.len() < 2 {
        return None;
    }
    Some(IntentResult {
        mode: QueryMode::Traverse,
        slot_text: rest.to_string(),
        scope_hint: None,
        pattern_name: "traverse_what_is",
    })
}

// "tell me about X"
fn try_describe_tell(input: &str) -> Option<IntentResult> {
    let rest = input.strip_prefix("tell me about ")?;
    let rest = non_empty(rest)?;
    let rest = rest.strip_prefix("the ").unwrap_or(rest);
    let rest = non_empty(rest)?;
    Some(IntentResult {
        mode: QueryMode::Describe,
        slot_text: rest.to_string(),
        scope_hint: None,
        pattern_name: "describe_tell",
    })
}

// "describe X"
fn try_describe_explicit(input: &str) -> Option<IntentResult> {
    let rest = input.strip_prefix("describe ")?;
    let rest = non_empty(rest)?;
    Some(IntentResult {
        mode: QueryMode::Describe,
        slot_text: rest.to_string(),
        scope_hint: None,
        pattern_name: "describe_explicit",
    })
}

// "what is X" — catch-all for "what is" without possessive
fn try_describe_what_is(input: &str) -> Option<IntentResult> {
    let rest = input.strip_prefix("what is ")?;
    let rest = non_empty(rest)?;
    Some(IntentResult {
        mode: QueryMode::Describe,
        slot_text: rest.to_string(),
        scope_hint: None,
        pattern_name: "describe_what_is",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_trailing_punctuation() {
        assert_eq!(normalize("What is Mars?"), "what is mars");
        assert_eq!(normalize("Tell me about the Sun!"), "tell me about the sun");
        assert_eq!(normalize("  hello   world  "), "hello world");
    }

    #[test]
    fn detect_tell_me_about() {
        let r = detect_intent("tell me about Mars").unwrap();
        assert!(matches!(r.mode, QueryMode::Describe));
        assert_eq!(r.slot_text, "mars");
        assert_eq!(r.pattern_name, "describe_tell");
    }

    #[test]
    fn detect_tell_me_about_the() {
        let r = detect_intent("Tell me about the Sun").unwrap();
        assert!(matches!(r.mode, QueryMode::Describe));
        assert_eq!(r.slot_text, "sun");
    }

    #[test]
    fn detect_describe_explicit() {
        let r = detect_intent("describe Surya").unwrap();
        assert!(matches!(r.mode, QueryMode::Describe));
        assert_eq!(r.slot_text, "surya");
        assert_eq!(r.pattern_name, "describe_explicit");
    }

    #[test]
    fn detect_what_is() {
        let r = detect_intent("what is Mars?").unwrap();
        assert!(matches!(r.mode, QueryMode::Describe));
        assert_eq!(r.slot_text, "mars");
        assert_eq!(r.pattern_name, "describe_what_is");
    }

    #[test]
    fn detect_what_does_traverse() {
        let r = detect_intent("what does Mars rule").unwrap();
        assert!(matches!(r.mode, QueryMode::Traverse));
        assert_eq!(r.slot_text, "mars rule");
        assert_eq!(r.pattern_name, "traverse_does");
    }

    #[test]
    fn detect_what_does_single_word_no_match() {
        // "what does Mars" alone has no predicate — shouldn't match traverse
        assert!(detect_intent("what does Mars").is_none());
    }

    #[test]
    fn detect_possessive() {
        let r = detect_intent("what is Mars's exaltation?").unwrap();
        assert!(matches!(r.mode, QueryMode::Traverse));
        assert_eq!(r.slot_text, "mars exaltation");
        assert_eq!(r.pattern_name, "traverse_possessive");
    }

    #[test]
    fn detect_similar_to() {
        let r = detect_intent("similar to Mars").unwrap();
        assert!(matches!(r.mode, QueryMode::Similar));
        assert_eq!(r.slot_text, "mars");
        assert_eq!(r.pattern_name, "similar");
    }

    #[test]
    fn detect_related_to() {
        let r = detect_intent("what is related to Mars?").unwrap();
        assert!(matches!(r.mode, QueryMode::Similar));
        assert_eq!(r.slot_text, "mars");
    }

    #[test]
    fn detect_what_are_search() {
        let r = detect_intent("what planets are fire?").unwrap();
        assert!(matches!(r.mode, QueryMode::Search));
        assert_eq!(r.slot_text, "planets fire");
        assert_eq!(r.pattern_name, "search_what_are");
    }

    #[test]
    fn detect_find_where() {
        let r = detect_intent("find grahas where element is fire").unwrap();
        assert!(matches!(r.mode, QueryMode::Search));
        assert_eq!(r.slot_text, "grahas element is fire");
        assert_eq!(r.pattern_name, "search_where");
    }

    #[test]
    fn detect_tradition_say() {
        let r = detect_intent("what does parashara say about Mars?").unwrap();
        assert!(matches!(r.mode, QueryMode::Describe));
        assert_eq!(r.scope_hint, Some("parashara".to_string()));
        assert_eq!(r.slot_text, "mars");
        assert_eq!(r.pattern_name, "tradition_say");
    }

    #[test]
    fn detect_tradition_according() {
        let r = detect_intent("according to BPHS, what does Mars rule?").unwrap();
        assert!(matches!(r.mode, QueryMode::Traverse));
        assert_eq!(r.scope_hint, Some("bphs".to_string()));
        assert_eq!(r.slot_text, "mars rule");
    }

    #[test]
    fn detect_no_match() {
        assert!(detect_intent("frobnicator xyzzy").is_none());
    }

    #[test]
    fn detect_empty_input() {
        assert!(detect_intent("").is_none());
        assert!(detect_intent("   ").is_none());
    }

    #[test]
    fn detect_pattern_only_no_slot() {
        assert!(detect_intent("tell me about").is_none());
        assert!(detect_intent("describe").is_none());
        assert!(detect_intent("what is").is_none());
        assert!(detect_intent("similar to").is_none());
    }

    #[test]
    fn detect_pramana_from_with_keyword() {
        let r = detect_intent("show claims from shabda pramana about Mars").unwrap();
        assert!(matches!(r.mode, QueryMode::Describe));
        assert_eq!(r.scope_hint, Some("shabda".to_string()));
        assert_eq!(r.slot_text, "mars");
        assert_eq!(r.pattern_name, "pramana_from");
    }

    #[test]
    fn detect_pramana_from_without_show() {
        let r = detect_intent("claims from anumana about Mars").unwrap();
        assert!(matches!(r.mode, QueryMode::Describe));
        assert_eq!(r.scope_hint, Some("anumana".to_string()));
        assert_eq!(r.slot_text, "mars");
    }

    #[test]
    fn detect_pramana_from_with_inner_intent() {
        let r = detect_intent("show claims from shabda pramana about what does mars rule").unwrap();
        assert!(matches!(r.mode, QueryMode::Traverse));
        assert_eq!(r.scope_hint, Some("shabda".to_string()));
        assert_eq!(r.slot_text, "mars rule");
    }

    #[test]
    fn detect_pramana_from_comma_form() {
        let r = detect_intent("from shabda pramana, describe Mars").unwrap();
        assert!(matches!(r.mode, QueryMode::Describe));
        assert_eq!(r.scope_hint, Some("shabda".to_string()));
        assert_eq!(r.slot_text, "mars");
    }
}
