use vidya_core::query::{
    AnnotatedTriple, DescribeResult, ProvenanceResult, SearchResult, SimilarityResult,
    TraverseResult, VocabResult,
};

pub fn fmt_describe(r: &DescribeResult) -> String {
    let mut out = String::new();

    let heading = r.label.as_deref().unwrap_or(&r.iri);
    out.push_str(&format!("  {heading}\n"));
    if !r.types.is_empty() {
        out.push_str(&format!("  types: {}\n", r.types.join(", ")));
    }
    out.push('\n');

    if !r.properties.is_empty() {
        let max_pred = r
            .properties
            .iter()
            .map(|p| p.predicate.len())
            .max()
            .unwrap_or(0);

        for pv in &r.properties {
            out.push_str(&format!(
                "  {:<width$}  {}\n",
                pv.predicate,
                pv.value,
                width = max_pred
            ));
        }
    }

    if !r.annotated_triples.is_empty() {
        out.push('\n');
        for at in &r.annotated_triples {
            fmt_annotated_triple(&mut out, at);
        }
    }

    out
}

fn fmt_annotated_triple(out: &mut String, at: &AnnotatedTriple) {
    out.push_str(&format!("  {} -> {}\n", at.predicate, at.object));

    if !at.annotations.is_empty() {
        for a in &at.annotations {
            out.push_str(&format!("    {}: {}\n", a.predicate, a.value));
        }
    }

    for p in &at.provenance {
        out.push_str(&format!(
            "    [{}, {}, {}, confidence={}]\n",
            p.tradition, p.source, p.pramana, p.confidence
        ));
    }
}

pub fn fmt_search(r: &SearchResult) -> String {
    let mut out = String::new();

    if r.entities.is_empty() {
        out.push_str("  (no results)\n");
        return out;
    }

    let max_name = r.entities.iter().map(|e| e.name.len()).max().unwrap_or(0);

    for hit in &r.entities {
        out.push_str(&format!(
            "  {:<width$}  {}\n",
            hit.name,
            hit.label,
            width = max_name
        ));
    }

    out
}

pub fn fmt_traverse(r: &TraverseResult) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "  {} --{}-- (depth {})\n\n",
        r.origin, r.predicate, r.max_depth
    ));

    if r.entities.is_empty() {
        out.push_str("  (no results)\n");
        return out;
    }

    for hit in &r.entities {
        let indent = "  ".repeat(hit.depth as usize);
        let label = hit.label.as_deref().unwrap_or(&hit.iri);
        out.push_str(&format!("  {indent}{label}\n"));
    }

    out
}

pub fn fmt_provenance(r: &ProvenanceResult) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "  {} {} {}\n\n",
        r.subject, r.predicate, r.object
    ));

    if r.assertions.is_empty() {
        out.push_str("  (no provenance)\n");
        return out;
    }

    for a in &r.assertions {
        out.push_str(&format!("  tradition:  {}\n", a.tradition));
        out.push_str(&format!("  source:     {}\n", a.source));
        out.push_str(&format!("  pramana:    {}\n", a.pramana));
        out.push_str(&format!("  confidence: {}\n", a.confidence));
        out.push('\n');
    }

    out
}

pub fn fmt_similarity(r: &SimilarityResult) -> String {
    let mut out = String::new();

    out.push_str(&format!("  {}\n\n", r.query));

    if r.matches.is_empty() {
        out.push_str("  (no results)\n");
        return out;
    }

    let max_label = r.matches.iter().map(|m| m.label.len()).max().unwrap_or(0);

    for (i, m) in r.matches.iter().enumerate() {
        out.push_str(&format!(
            "  {:>2}. {:<w$}  {:.3}\n",
            i + 1,
            m.label,
            m.score,
            w = max_label
        ));
    }

    out
}

pub fn fmt_vocab(r: &VocabResult) -> String {
    let mut out = String::new();

    out.push_str("Types:\n");
    if r.types.is_empty() {
        out.push_str("  (none)\n");
    } else {
        let max = r.types.iter().map(|t| t.name.len()).max().unwrap_or(0);
        for t in &r.types {
            out.push_str(&format!("  {:<w$}  {}\n", t.name, t.iri, w = max));
        }
    }

    out.push_str("\nEntities:\n");
    if r.entities.is_empty() {
        out.push_str("  (none)\n");
    } else {
        let max = r
            .entities
            .iter()
            .map(|e| e.names.join(", ").len())
            .max()
            .unwrap_or(0);
        for e in &r.entities {
            let names = e.names.join(", ");
            out.push_str(&format!("  {:<w$}  {}\n", names, e.iri, w = max));
        }
    }

    out.push_str("\nPredicates:\n");
    if r.predicates.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for p in &r.predicates {
            out.push_str(&format!("  {}\n", p.name));
        }
    }

    out.push_str("\nValues (filter keys):\n");
    if r.values.is_empty() {
        out.push_str("  (none)\n");
    } else {
        let max = r.values.iter().map(|v| v.value.len()).max().unwrap_or(0);
        for v in &r.values {
            out.push_str(&format!(
                "  {:<w$}  \u{2192} {}\n",
                v.value,
                v.predicate,
                w = max
            ));
        }
    }

    out
}
