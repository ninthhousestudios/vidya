pub const VIDYA_BASE: &str = "http://vidya.ninthhouse.studio/ontology/";
pub const DOMAIN_BASE: &str = "http://vidya.ninthhouse.studio/domain/";
pub const BASE_ONTOLOGY_VERSION: &str = "0.2.0";
pub const EXPECTED_PRAMANA_COUNT: i64 = 6;

pub const VIDYA_TTL: &str = include_str!("../../ontology/vidya.ttl");

pub fn domain_graph_iri(domain: &str) -> String {
    format!("{DOMAIN_BASE}{domain}/")
}

pub fn domain_iri(domain: &str, local: &str) -> String {
    format!("{DOMAIN_BASE}{domain}/{local}")
}

pub fn vidya_iri(local: &str) -> String {
    format!("{VIDYA_BASE}{local}")
}

/// Resolve a short name to a full IRI.
/// "vidya:X" → vidya base; otherwise → domain-scoped.
pub fn resolve_iri(name: &str, domain: &str) -> String {
    if let Some(local) = name.strip_prefix("vidya:") {
        vidya_iri(local)
    } else {
        domain_iri(domain, name)
    }
}
