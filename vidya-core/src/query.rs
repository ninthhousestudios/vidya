use std::collections::{BTreeMap, HashSet};

use oxigraph::model::{NamedNodeRef, Term};
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use serde::Serialize;

use crate::error::{Result, VidyaError};
use crate::ontology;
use crate::store::KnowledgeStore;

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct DescribeResult {
    pub iri: String,
    pub label: Option<String>,
    pub types: Vec<String>,
    pub properties: Vec<PropertyValue>,
    pub annotated_triples: Vec<AnnotatedTriple>,
}

#[derive(Debug, Serialize)]
pub struct PropertyValue {
    pub predicate: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct AnnotatedTriple {
    pub predicate: String,
    pub object: String,
    pub annotations: Vec<PropertyValue>,
    pub provenance: Option<Provenance>,
}

#[derive(Debug, Serialize)]
pub struct Provenance {
    pub tradition: String,
    pub source: String,
    pub pramana: String,
    pub confidence: String,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub entities: Vec<SearchHit>,
}

#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub iri: String,
    pub name: String,
    pub label: String,
}

#[derive(Debug, Serialize)]
pub struct TraverseResult {
    pub origin: String,
    pub predicate: String,
    pub max_depth: u32,
    pub entities: Vec<TraverseHit>,
}

#[derive(Debug, Serialize)]
pub struct TraverseHit {
    pub iri: String,
    pub label: Option<String>,
    pub depth: u32,
}

#[derive(Debug, Serialize)]
pub struct ProvenanceResult {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub assertions: Vec<Provenance>,
}

#[derive(Debug, Default)]
pub struct ProvenanceFilter {
    pub tradition: Option<String>,
    pub pramana: Option<String>,
}

const MAX_TRAVERSE_DEPTH: u32 = 10;

// ---------------------------------------------------------------------------
// SPARQL builder
// ---------------------------------------------------------------------------

pub(crate) struct SparqlBuilder {
    prefixes: Vec<(String, String)>,
    select_vars: Vec<String>,
    body_clauses: Vec<String>,
    optional_clauses: Vec<String>,
    filter_clauses: Vec<String>,
    order_by: Option<String>,
    distinct: bool,
}

#[allow(dead_code)]
impl SparqlBuilder {
    pub fn new() -> Self {
        Self {
            prefixes: Vec::new(),
            select_vars: Vec::new(),
            body_clauses: Vec::new(),
            optional_clauses: Vec::new(),
            filter_clauses: Vec::new(),
            order_by: None,
            distinct: false,
        }
    }

    pub fn add_prefix(&mut self, prefix: &str, iri: &str) {
        self.prefixes.push((prefix.to_string(), iri.to_string()));
    }

    pub fn add_select(&mut self, var: &str) {
        self.select_vars.push(var.to_string());
    }

    pub fn add_body(&mut self, clause: &str) {
        self.body_clauses.push(clause.to_string());
    }

    pub fn add_optional(&mut self, clause: &str) {
        self.optional_clauses.push(clause.to_string());
    }

    pub fn add_filter(&mut self, clause: &str) {
        self.filter_clauses.push(clause.to_string());
    }

    pub fn set_order_by(&mut self, expr: &str) {
        self.order_by = Some(expr.to_string());
    }

    pub fn set_distinct(&mut self) {
        self.distinct = true;
    }

    pub fn build(&self) -> String {
        let mut out = String::new();
        for (prefix, iri) in &self.prefixes {
            out.push_str(&format!("PREFIX {prefix}: <{iri}>\n"));
        }
        let kw = if self.distinct { "SELECT DISTINCT" } else { "SELECT" };
        out.push_str(&format!("{kw} {}\n", self.select_vars.join(" ")));
        out.push_str("WHERE {\n");
        for clause in &self.body_clauses {
            out.push_str("  ");
            out.push_str(clause);
            out.push('\n');
        }
        for clause in &self.optional_clauses {
            out.push_str("  ");
            out.push_str(clause);
            out.push('\n');
        }
        for clause in &self.filter_clauses {
            out.push_str("  ");
            out.push_str(clause);
            out.push('\n');
        }
        out.push_str("}\n");
        if let Some(ref order) = self.order_by {
            out.push_str(&format!("ORDER BY {order}\n"));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// IRI shortening
// ---------------------------------------------------------------------------

const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

fn shorten_iri(iri: &str, domain: &str) -> String {
    let domain_base = format!("{}{}/", ontology::DOMAIN_BASE, domain);
    if let Some(local) = iri.strip_prefix(&domain_base) {
        return local.to_string();
    }
    if let Some(local) = iri.strip_prefix(ontology::VIDYA_BASE) {
        return format!("vidya:{local}");
    }
    if let Some(local) = iri.strip_prefix(RDFS) {
        return format!("rdfs:{local}");
    }
    if let Some(local) = iri.strip_prefix(RDF) {
        return format!("rdf:{local}");
    }
    if let Some(local) = iri.strip_prefix(XSD) {
        return format!("xsd:{local}");
    }
    iri.to_string()
}

fn term_to_display(term: &Term, domain: &str) -> String {
    match term {
        Term::NamedNode(nn) => shorten_iri(nn.as_str(), domain),
        Term::Literal(lit) => lit.value().to_string(),
        Term::BlankNode(bn) => format!("_:{}", bn.as_str()),
        Term::Triple(_) => term.to_string(),
    }
}

// ---------------------------------------------------------------------------
// SPARQL string escaping
// ---------------------------------------------------------------------------

fn escape_sparql_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// IRI validation
// ---------------------------------------------------------------------------

fn validate_iri(iri: &str) -> Result<()> {
    NamedNodeRef::new(iri).map_err(|_| {
        VidyaError::InvalidArgument(format!("invalid IRI: {iri}"))
    })?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Query execution helper
// ---------------------------------------------------------------------------

fn execute_select(
    store: &KnowledgeStore,
    query: &str,
) -> Result<(Vec<String>, Vec<Vec<Option<Term>>>)> {
    match SparqlEvaluator::new()
        .parse_query(query)
        .map_err(|e| VidyaError::Internal(e.to_string()))?
        .on_store(store.inner())
        .execute()?
    {
        QueryResults::Solutions(solutions) => {
            let vars: Vec<String> = solutions
                .variables()
                .iter()
                .map(|v| v.as_str().to_string())
                .collect();
            let mut rows = Vec::new();
            for solution in solutions {
                let solution = solution?;
                let row: Vec<Option<Term>> = vars
                    .iter()
                    .map(|v| solution.get(v.as_str()).cloned())
                    .collect();
                rows.push(row);
            }
            Ok((vars, rows))
        }
        _ => Err(VidyaError::Internal("expected SELECT results".into())),
    }
}

fn get_col(vars: &[String], name: &str) -> Option<usize> {
    vars.iter().position(|v| v == name)
}

fn cell_str(row: &[Option<Term>], idx: usize, domain: &str) -> Option<String> {
    row.get(idx)
        .and_then(|t| t.as_ref())
        .map(|t| term_to_display(t, domain))
}

// ---------------------------------------------------------------------------
// Provenance filter injection
// ---------------------------------------------------------------------------

fn apply_provenance_filter(
    builder: &mut SparqlBuilder,
    subject_expr: &str,
    pred_expr: &str,
    obj_expr: &str,
    filter: &ProvenanceFilter,
) {
    if filter.tradition.is_none() && filter.pramana.is_none() {
        return;
    }
    builder.add_body(&format!(
        "<< {subject_expr} {pred_expr} {obj_expr} >> vidya:assertedBy ?_pf_assertion ."
    ));
    if let Some(ref trad) = filter.tradition {
        builder.add_body(&format!("?_pf_assertion vidya:tradition <{trad}> ."));
    }
    if let Some(ref pram) = filter.pramana {
        builder.add_body(&format!("?_pf_assertion vidya:pramana <{pram}> ."));
    }
}

fn resolve_object_term(object: &str, domain: &str) -> Result<String> {
    let candidate = ontology::resolve_iri(object, domain);
    if validate_iri(&candidate).is_ok() {
        Ok(format!("<{candidate}>"))
    } else {
        Ok(format!("\"{}\"", escape_sparql_string(object)))
    }
}

// ---------------------------------------------------------------------------
// Provenance
// ---------------------------------------------------------------------------

pub fn provenance(
    store: &KnowledgeStore,
    domain: &str,
    subject: &str,
    predicate: &str,
    object: &str,
    filter: &ProvenanceFilter,
) -> Result<ProvenanceResult> {
    let subject_iri = ontology::resolve_iri(subject, domain);
    let pred_iri = ontology::resolve_iri(predicate, domain);
    let graph_iri = ontology::domain_graph_iri(domain);
    validate_iri(&subject_iri)?;
    validate_iri(&pred_iri)?;
    validate_iri(&graph_iri)?;
    let obj_term = resolve_object_term(object, domain)?;

    let mut b = SparqlBuilder::new();
    b.add_prefix("vidya", ontology::VIDYA_BASE);
    b.add_select("?trad");
    b.add_select("?src");
    b.add_select("?pramana");
    b.add_select("?confidence");
    b.add_body(&format!("GRAPH <{graph_iri}> {{"));
    b.add_body(&format!(
        "  << <{subject_iri}> <{pred_iri}> {obj_term} >> vidya:assertedBy ?assertion ."
    ));
    b.add_body("  ?assertion vidya:tradition ?trad ;");
    b.add_body("             vidya:source ?src ;");
    b.add_body("             vidya:pramana ?pramana ;");
    b.add_body("             vidya:confidence ?confidence .");
    if let Some(ref trad) = filter.tradition {
        validate_iri(trad)?;
        b.add_filter(&format!("FILTER(?trad = <{trad}>)"));
    }
    if let Some(ref pram) = filter.pramana {
        validate_iri(pram)?;
        b.add_filter(&format!("FILTER(?pramana = <{pram}>)"));
    }
    b.add_body("}");
    let query = b.build();

    let (vars, rows) = execute_select(store, &query)?;
    let itrad = get_col(&vars, "trad").unwrap();
    let isrc = get_col(&vars, "src").unwrap();
    let ipram = get_col(&vars, "pramana").unwrap();
    let iconf = get_col(&vars, "confidence").unwrap();

    let assertions: Vec<Provenance> = rows
        .iter()
        .map(|row| Provenance {
            tradition: cell_str(row, itrad, domain).unwrap_or_default(),
            source: cell_str(row, isrc, domain).unwrap_or_default(),
            pramana: cell_str(row, ipram, domain).unwrap_or_default(),
            confidence: cell_str(row, iconf, domain).unwrap_or_default(),
        })
        .collect();

    Ok(ProvenanceResult {
        subject: shorten_iri(&subject_iri, domain),
        predicate: shorten_iri(&pred_iri, domain),
        object: object.to_string(),
        assertions,
    })
}

// ---------------------------------------------------------------------------
// Traverse
// ---------------------------------------------------------------------------

pub fn traverse(
    store: &KnowledgeStore,
    domain: &str,
    subject: &str,
    predicate: &str,
    depth: u32,
    filter: &ProvenanceFilter,
) -> Result<TraverseResult> {
    if depth > MAX_TRAVERSE_DEPTH {
        return Err(VidyaError::InvalidArgument(format!(
            "depth {depth} exceeds maximum {MAX_TRAVERSE_DEPTH}"
        )));
    }

    let subject_iri = ontology::resolve_iri(subject, domain);
    let pred_iri = ontology::resolve_iri(predicate, domain);
    let graph_iri = ontology::domain_graph_iri(domain);
    validate_iri(&subject_iri)?;
    validate_iri(&pred_iri)?;
    validate_iri(&graph_iri)?;

    if let Some(ref trad) = filter.tradition {
        validate_iri(trad)?;
    }
    if let Some(ref pram) = filter.pramana {
        validate_iri(pram)?;
    }

    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(subject_iri.clone());
    let mut frontier: Vec<String> = vec![subject_iri.clone()];
    let mut entities: Vec<TraverseHit> = Vec::new();

    for d in 1..=depth {
        if frontier.is_empty() {
            break;
        }

        let values: String = frontier
            .iter()
            .map(|iri| format!("<{iri}>"))
            .collect::<Vec<_>>()
            .join(" ");

        let mut b = SparqlBuilder::new();
        b.add_prefix("vidya", ontology::VIDYA_BASE);
        b.add_prefix("rdfs", RDFS);
        b.add_select("?obj");
        b.add_select("?label");
        b.add_body(&format!("GRAPH <{graph_iri}> {{"));
        b.add_body(&format!("  VALUES ?start {{ {values} }}"));
        b.add_body(&format!("  ?start <{pred_iri}> ?obj ."));
        apply_provenance_filter(&mut b, "?start", &format!("<{pred_iri}>"), "?obj", filter);
        b.add_body("  OPTIONAL { ?obj rdfs:label ?label . }");
        b.add_body("}");
        let query = b.build();

        let (vars, rows) = execute_select(store, &query)?;
        let io = get_col(&vars, "obj").unwrap();
        let il = get_col(&vars, "label").unwrap();

        let mut next_frontier = Vec::new();
        for row in &rows {
            if let Some(Term::NamedNode(nn)) = row.get(io).and_then(|t| t.as_ref()) {
                let iri = nn.as_str().to_string();
                if visited.insert(iri.clone()) {
                    let label = cell_str(row, il, domain);
                    entities.push(TraverseHit {
                        iri: shorten_iri(&iri, domain),
                        label,
                        depth: d,
                    });
                    next_frontier.push(iri);
                }
            }
        }
        frontier = next_frontier;
    }

    Ok(TraverseResult {
        origin: shorten_iri(&subject_iri, domain),
        predicate: shorten_iri(&pred_iri, domain),
        max_depth: depth,
        entities,
    })
}

// ---------------------------------------------------------------------------
// Describe
// ---------------------------------------------------------------------------

pub fn describe(store: &KnowledgeStore, domain: &str, subject: &str, filter: &ProvenanceFilter) -> Result<DescribeResult> {
    let subject_iri = ontology::resolve_iri(subject, domain);
    let graph_iri = ontology::domain_graph_iri(domain);
    validate_iri(&subject_iri)?;
    validate_iri(&graph_iri)?;

    // Query 1: all regular triples about the subject
    let mut b = SparqlBuilder::new();
    b.add_select("?p");
    b.add_select("?o");
    b.add_body(&format!("GRAPH <{graph_iri}> {{"));
    b.add_body(&format!("  <{subject_iri}> ?p ?o ."));
    b.add_body("}");
    let q1 = b.build();

    let (vars1, rows1) = execute_select(store, &q1)?;
    if rows1.is_empty() {
        return Err(VidyaError::NotFound(format!(
            "{subject} not found in domain {domain}"
        )));
    }

    let ip = get_col(&vars1, "p").unwrap();
    let io = get_col(&vars1, "o").unwrap();

    let mut label: Option<String> = None;
    let mut types: Vec<String> = Vec::new();
    let mut properties: Vec<PropertyValue> = Vec::new();

    for row in &rows1 {
        let pred = cell_str(row, ip, domain).unwrap_or_default();
        let obj = cell_str(row, io, domain).unwrap_or_default();

        if pred == "rdf:type" {
            types.push(obj);
        } else if pred == "rdfs:label" {
            label = Some(obj);
        } else {
            properties.push(PropertyValue {
                predicate: pred,
                value: obj,
            });
        }
    }

    // Query 2: all annotations on quoted triples where subject is the subject
    let mut b2 = SparqlBuilder::new();
    b2.add_prefix("vidya", ontology::VIDYA_BASE);
    b2.add_select("?p");
    b2.add_select("?o");
    b2.add_select("?annot_p");
    b2.add_select("?annot_o");
    b2.add_body(&format!("GRAPH <{graph_iri}> {{"));
    b2.add_body(&format!("  << <{subject_iri}> ?p ?o >> ?annot_p ?annot_o ."));
    b2.add_body("}");
    let q2 = b2.build();

    // Query 3: provenance details (assertedBy with tradition/source/pramana/confidence)
    let mut b3 = SparqlBuilder::new();
    b3.add_prefix("vidya", ontology::VIDYA_BASE);
    b3.add_select("?p");
    b3.add_select("?o");
    b3.add_select("?trad");
    b3.add_select("?src");
    b3.add_select("?pramana");
    b3.add_select("?confidence");
    b3.add_body(&format!("GRAPH <{graph_iri}> {{"));
    b3.add_body(&format!("  << <{subject_iri}> ?p ?o >> vidya:assertedBy ?assertion ."));
    b3.add_body("  ?assertion vidya:tradition ?trad ;");
    b3.add_body("             vidya:source ?src ;");
    b3.add_body("             vidya:pramana ?pramana ;");
    b3.add_body("             vidya:confidence ?confidence .");
    if let Some(ref trad) = filter.tradition {
        validate_iri(trad)?;
        b3.add_filter(&format!("FILTER(?trad = <{trad}>)"));
    }
    if let Some(ref pram) = filter.pramana {
        validate_iri(pram)?;
        b3.add_filter(&format!("FILTER(?pramana = <{pram}>)"));
    }
    b3.add_body("}");
    let q3 = b3.build();

    let (vars2, rows2) = execute_select(store, &q2)?;
    let (vars3, rows3) = execute_select(store, &q3)?;

    // Build annotation map: (pred, obj) → Vec<PropertyValue>
    // (excludes assertedBy since that's handled separately as provenance)
    let ip2 = get_col(&vars2, "p").unwrap();
    let io2 = get_col(&vars2, "o").unwrap();
    let iap = get_col(&vars2, "annot_p").unwrap();
    let iao = get_col(&vars2, "annot_o").unwrap();

    let mut annot_map: BTreeMap<(String, String), Vec<PropertyValue>> = BTreeMap::new();

    for row in &rows2 {
        let pred = cell_str(row, ip2, domain).unwrap_or_default();
        let obj = cell_str(row, io2, domain).unwrap_or_default();
        let annot_pred = cell_str(row, iap, domain).unwrap_or_default();
        let annot_obj = cell_str(row, iao, domain).unwrap_or_default();

        if annot_pred == "vidya:assertedBy" {
            continue;
        }

        annot_map
            .entry((pred, obj))
            .or_default()
            .push(PropertyValue {
                predicate: annot_pred,
                value: annot_obj,
            });
    }

    // Build provenance map: (pred, obj) → Provenance
    let ip3 = get_col(&vars3, "p").unwrap();
    let io3 = get_col(&vars3, "o").unwrap();
    let itrad = get_col(&vars3, "trad").unwrap();
    let isrc = get_col(&vars3, "src").unwrap();
    let ipram = get_col(&vars3, "pramana").unwrap();
    let iconf = get_col(&vars3, "confidence").unwrap();

    let mut prov_map: BTreeMap<(String, String), Provenance> = BTreeMap::new();

    for row in &rows3 {
        let pred = cell_str(row, ip3, domain).unwrap_or_default();
        let obj = cell_str(row, io3, domain).unwrap_or_default();
        let trad = cell_str(row, itrad, domain).unwrap_or_default();
        let src = cell_str(row, isrc, domain).unwrap_or_default();
        let pramana = cell_str(row, ipram, domain).unwrap_or_default();
        let confidence = cell_str(row, iconf, domain).unwrap_or_default();

        prov_map.insert(
            (pred, obj),
            Provenance {
                tradition: trad,
                source: src,
                pramana,
                confidence,
            },
        );
    }

    // Merge annotations + provenance into annotated triples
    let mut all_keys: Vec<(String, String)> = annot_map.keys().cloned().collect();
    for key in prov_map.keys() {
        if !all_keys.contains(key) {
            all_keys.push(key.clone());
        }
    }
    all_keys.sort();

    let annotated_triples: Vec<AnnotatedTriple> = all_keys
        .into_iter()
        .map(|(pred, obj)| {
            let annotations = annot_map.remove(&(pred.clone(), obj.clone())).unwrap_or_default();
            let provenance = prov_map.remove(&(pred.clone(), obj.clone()));
            AnnotatedTriple {
                predicate: pred,
                object: obj,
                annotations,
                provenance,
            }
        })
        .collect();

    Ok(DescribeResult {
        iri: shorten_iri(&subject_iri, domain),
        label,
        types,
        properties,
        annotated_triples,
    })
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

pub fn search(
    store: &KnowledgeStore,
    domain: &str,
    kind: &str,
    filters: &[(String, String)],
    prov_filter: &ProvenanceFilter,
) -> Result<SearchResult> {
    let graph_iri = ontology::domain_graph_iri(domain);
    let kind_iri = ontology::resolve_iri(kind, domain);
    validate_iri(&graph_iri)?;
    validate_iri(&kind_iri)?;

    for (attr, _) in filters {
        let attr_iri = ontology::resolve_iri(attr, domain);
        validate_iri(&attr_iri)?;
    }

    let mut b = SparqlBuilder::new();
    b.add_prefix("rdfs", RDFS);
    b.add_prefix("vidya", ontology::VIDYA_BASE);
    b.add_select("?entity");
    b.add_select("?label");
    b.add_body(&format!("GRAPH <{graph_iri}> {{"));
    b.add_body(&format!("  ?entity a <{kind_iri}> ."));
    b.add_body("  ?entity rdfs:label ?label .");

    for (attr, val) in filters {
        let attr_iri = ontology::resolve_iri(attr, domain);
        let escaped = escape_sparql_string(val);
        b.add_body(&format!("  ?entity <{attr_iri}> \"{escaped}\" ."));
    }

    if prov_filter.tradition.is_some() || prov_filter.pramana.is_some() {
        b.add_body("  ?entity ?_pf_p ?_pf_o .");
        apply_provenance_filter(&mut b, "?entity", "?_pf_p", "?_pf_o", prov_filter);
        b.set_distinct();
    }

    b.add_body("}");
    b.set_order_by("?label");
    let query = b.build();

    let (vars, rows) = execute_select(store, &query)?;
    let ie = get_col(&vars, "entity").unwrap();
    let il = get_col(&vars, "label").unwrap();

    let entities: Vec<SearchHit> = rows
        .iter()
        .filter_map(|row| {
            let iri_short = cell_str(row, ie, domain)?;
            let lbl = cell_str(row, il, domain)?;
            Some(SearchHit {
                name: iri_short.clone(),
                iri: iri_short,
                label: lbl,
            })
        })
        .collect();

    Ok(SearchResult { entities })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorten_domain_iri() {
        assert_eq!(
            shorten_iri(
                "http://vidya.ninthhouse.studio/domain/jyotish/surya",
                "jyotish"
            ),
            "surya"
        );
    }

    #[test]
    fn shorten_vidya_iri() {
        assert_eq!(
            shorten_iri(
                "http://vidya.ninthhouse.studio/ontology/assertedBy",
                "jyotish"
            ),
            "vidya:assertedBy"
        );
    }

    #[test]
    fn shorten_rdfs_iri() {
        assert_eq!(
            shorten_iri(
                "http://www.w3.org/2000/01/rdf-schema#label",
                "jyotish"
            ),
            "rdfs:label"
        );
    }

    #[test]
    fn escape_special_chars() {
        assert_eq!(escape_sparql_string(r#"he said "hi""#), r#"he said \"hi\""#);
        assert_eq!(escape_sparql_string("line\nnew"), "line\\nnew");
    }

    #[test]
    fn builder_assembles_query() {
        let mut b = SparqlBuilder::new();
        b.add_prefix("rdfs", RDFS);
        b.add_select("?s");
        b.add_body("?s a <http://example.org/Foo> .");
        b.set_order_by("?s");
        let q = b.build();
        assert!(q.contains("PREFIX rdfs:"));
        assert!(q.contains("SELECT ?s"));
        assert!(q.contains("WHERE {"));
        assert!(q.contains("ORDER BY ?s"));
    }
}
