use serial_test::serial;
use serde_json::json;
use std::path::Path;
use vidya::tools;

async fn test_pool() -> sqlx::PgPool {
    let _ = dotenvy::dotenv();
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/vidya".into());
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("connect to test database")
}

async fn cleanup(pool: &sqlx::PgPool, domain_slug: &str) {
    let _ = sqlx::query(
        "DELETE FROM assertions WHERE claim_id IN \
         (SELECT id FROM claims WHERE domain_id = (SELECT id FROM domains WHERE slug = $1))",
    )
    .bind(domain_slug)
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "DELETE FROM relations WHERE domain_id = (SELECT id FROM domains WHERE slug = $1)",
    )
    .bind(domain_slug)
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "DELETE FROM claims WHERE domain_id = (SELECT id FROM domains WHERE slug = $1)",
    )
    .bind(domain_slug)
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "DELETE FROM entities WHERE domain_id = (SELECT id FROM domains WHERE slug = $1)",
    )
    .bind(domain_slug)
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "DELETE FROM claim_templates WHERE domain_id = (SELECT id FROM domains WHERE slug = $1)",
    )
    .bind(domain_slug)
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "DELETE FROM relation_kinds WHERE domain_id = (SELECT id FROM domains WHERE slug = $1)",
    )
    .bind(domain_slug)
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "DELETE FROM entity_kinds WHERE domain_id = (SELECT id FROM domains WHERE slug = $1)",
    )
    .bind(domain_slug)
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "DELETE FROM traditions WHERE domain_id = (SELECT id FROM domains WHERE slug = $1)",
    )
    .bind(domain_slug)
    .execute(pool)
    .await;
    let _ = sqlx::query("DELETE FROM domains WHERE slug = $1")
        .bind(domain_slug)
        .execute(pool)
        .await;
}

async fn cleanup_sources(pool: &sqlx::PgPool, slugs: &[&str]) {
    for slug in slugs {
        let _ = sqlx::query("DELETE FROM sources WHERE slug = $1")
            .bind(*slug)
            .execute(pool)
            .await;
    }
}

async fn load_seed_file(pool: &sqlx::PgPool, path: &Path) -> tools::load::LoadOutput {
    let content = std::fs::read_to_string(path).expect("read seed file");
    let payload: serde_json::Value = serde_json::from_str(&content).expect("parse seed JSON");
    tools::load::handle(pool, tools::LoadArgs { payload })
        .await
        .expect("load should succeed")
}

#[tokio::test]
async fn load_domain_and_query_entity() {
    let pool = test_pool().await;
    let slug = "test-integration";

    cleanup(&pool, slug).await;

    let payload = json!({
        "domain": { "slug": slug, "title": "Integration Test Domain" },
        "entity_kinds": [
            { "slug": "vowel", "schema": null }
        ],
        "entities": [
            { "kind": "vowel", "name": "a", "attrs": { "class": "short", "type": "simple" } },
            { "kind": "vowel", "name": "ā", "attrs": { "class": "long", "type": "simple" } }
        ]
    });

    let load_result = tools::load::handle(
        &pool,
        tools::LoadArgs {
            payload: payload.clone(),
        },
    )
    .await
    .expect("load should succeed");

    assert_eq!(load_result.entities, 2);
    assert_eq!(load_result.entity_kinds, 1);

    // Query entity by name
    let get_result = tools::entity::handle(
        &pool,
        tools::EntityArgs {
            action: "get".into(),
            domain: slug.into(),
            kind: None,
            name: Some("a".into()),
            attrs: None,
        },
    )
    .await
    .expect("get entity should succeed");

    let entity = get_result.entity.expect("entity should exist");
    assert_eq!(entity.name, "a");
    assert_eq!(entity.attrs["class"], "short");

    // Idempotency — load again
    let load_result2 = tools::load::handle(
        &pool,
        tools::LoadArgs { payload },
    )
    .await
    .expect("idempotent load should succeed");

    assert_eq!(load_result2.entities, 2);

    // List entities by kind
    let list_result = tools::entity::handle(
        &pool,
        tools::EntityArgs {
            action: "list".into(),
            domain: slug.into(),
            kind: Some("vowel".into()),
            name: None,
            attrs: None,
        },
    )
    .await
    .expect("list should succeed");

    let entities = list_result.entities.expect("should have entities list");
    assert_eq!(entities.len(), 2);

    cleanup(&pool, slug).await;
}

#[tokio::test]
#[serial]
async fn load_vyakarana_seed() {
    let pool = test_pool().await;
    let slug = "vyakarana";
    let source_slugs = &["ashtadhyayi", "shiva-sutras"];

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, source_slugs).await;

    let result = load_seed_file(&pool, Path::new("seeds/vyakarana.json")).await;

    assert_eq!(result.domain, "vyakarana");
    assert_eq!(result.entity_kinds, 5);
    assert_eq!(result.relation_kinds, 3);
    assert_eq!(result.claim_templates, 3);
    assert_eq!(result.traditions, 3);
    assert_eq!(result.sources, 2);
    assert_eq!(result.entities, 44);
    assert_eq!(result.claims, 32);
    assert_eq!(result.assertions, 32);
    assert_eq!(result.relations, 0);

    // Idempotent reload
    let result2 = load_seed_file(&pool, Path::new("seeds/vyakarana.json")).await;
    assert_eq!(result2.domain, "vyakarana");
    assert_eq!(result2.entities, result.entities);
    assert_eq!(result2.claims, result.claims);

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, source_slugs).await;
}

#[tokio::test]
#[serial]
async fn load_jyotish_seed() {
    let pool = test_pool().await;
    let slug = "jyotish";
    let source_slugs = &["bphs", "phala-dipika", "jataka-parijata", "saravali"];

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, source_slugs).await;

    let result = load_seed_file(&pool, Path::new("seeds/jyotish.json")).await;

    assert_eq!(result.domain, "jyotish");
    assert_eq!(result.entity_kinds, 6);
    assert_eq!(result.relation_kinds, 9);
    assert_eq!(result.claim_templates, 7);
    assert_eq!(result.traditions, 3);
    assert_eq!(result.sources, 4);
    assert!(result.entities > 0);
    assert!(result.claims > 0);
    assert!(result.assertions > 0);
    assert!(result.relations > 0);

    // Idempotent reload
    let result2 = load_seed_file(&pool, Path::new("seeds/jyotish.json")).await;
    assert_eq!(result2.entities, result.entities);
    assert_eq!(result2.claims, result.claims);

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, source_slugs).await;
}

#[tokio::test]
async fn load_rejects_invalid_claim_params() {
    let pool = test_pool().await;
    let slug = "test-validation";

    cleanup(&pool, slug).await;

    let payload = json!({
        "domain": { "slug": slug, "title": "Validation Test" },
        "entity_kinds": [],
        "claim_templates": [
            {
                "slug": "typed_rule",
                "param_schema": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "count": { "type": "integer" }
                    },
                    "required": ["name", "count"]
                }
            }
        ],
        "claims": [
            {
                "template": "typed_rule",
                "params": { "name": "test", "count": "not-a-number" },
                "statement": "bad claim"
            }
        ]
    });

    let err = tools::load::handle(&pool, tools::LoadArgs { payload })
        .await
        .expect_err("should reject invalid params");
    let msg = err.to_string();
    assert!(msg.contains("count"), "error should mention the bad field: {msg}");

    cleanup(&pool, slug).await;
}

#[tokio::test]
async fn claim_create_rejects_invalid_params() {
    let pool = test_pool().await;
    let slug = "test-claim-validation";

    cleanup(&pool, slug).await;

    // Setup: load a domain with a claim_template that has param_schema
    let payload = json!({
        "domain": { "slug": slug, "title": "Claim Validation Test" },
        "entity_kinds": [],
        "claim_templates": [
            {
                "slug": "typed_rule",
                "param_schema": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "count": { "type": "integer" }
                    },
                    "required": ["name", "count"]
                }
            }
        ]
    });
    tools::load::handle(&pool, tools::LoadArgs { payload })
        .await
        .expect("setup load should succeed");

    // Act: create a claim with invalid params (count should be integer, not string)
    let err = tools::claim::handle(
        &pool,
        tools::ClaimArgs {
            action: "create".into(),
            domain: slug.into(),
            template: Some("typed_rule".into()),
            params: Some(json!({ "name": "test", "count": "not-a-number" })),
            statement: Some("bad claim".into()),
            status: None,
            tradition: None,
            source_ref: None,
            source_kind: None,
            pramana: None,
            confidence: None,
            id: None,
        },
    )
    .await
    .expect_err("should reject invalid params");

    let msg = err.to_string();
    assert!(
        msg.contains("count"),
        "error should mention the bad field: {msg}"
    );

    // Also verify valid params succeed
    let ok = tools::claim::handle(
        &pool,
        tools::ClaimArgs {
            action: "create".into(),
            domain: slug.into(),
            template: Some("typed_rule".into()),
            params: Some(json!({ "name": "test", "count": 42 })),
            statement: Some("good claim".into()),
            status: None,
            tradition: None,
            source_ref: None,
            source_kind: None,
            pramana: None,
            confidence: None,
            id: None,
        },
    )
    .await
    .expect("valid params should succeed");

    assert_eq!(ok.action, "created");

    cleanup(&pool, slug).await;
}

#[tokio::test]
async fn claim_status_transitions() {
    let pool = test_pool().await;
    let slug = "test-status-transitions";

    cleanup(&pool, slug).await;

    // Setup: domain with a claim_template
    let payload = json!({
        "domain": { "slug": slug, "title": "Status Transition Test" },
        "entity_kinds": [],
        "claim_templates": [
            { "slug": "rule", "param_schema": {} }
        ]
    });
    tools::load::handle(&pool, tools::LoadArgs { payload })
        .await
        .expect("setup load");

    // Create a claim with status "proposed"
    let created = tools::claim::handle(
        &pool,
        tools::ClaimArgs {
            action: "create".into(),
            domain: slug.into(),
            template: Some("rule".into()),
            params: Some(json!({})),
            statement: Some("test claim".into()),
            status: Some("proposed".into()),
            tradition: None,
            source_ref: None,
            source_kind: None,
            pramana: None,
            confidence: None,
            id: None,
        },
    )
    .await
    .expect("create claim");
    let claim_id = created.claim.unwrap().id.to_string();

    // proposed → active: allowed
    let updated = tools::claim::handle(
        &pool,
        tools::ClaimArgs {
            action: "update".into(),
            domain: slug.into(),
            template: None,
            params: None,
            statement: None,
            status: Some("active".into()),
            tradition: None,
            source_ref: None,
            source_kind: None,
            pramana: None,
            confidence: None,
            id: Some(claim_id.clone()),
        },
    )
    .await
    .expect("proposed → active should succeed");
    assert_eq!(updated.claim.as_ref().unwrap().status, "active");
    assert_eq!(updated.action, "updated");

    // active → historical: allowed
    let updated2 = tools::claim::handle(
        &pool,
        tools::ClaimArgs {
            action: "update".into(),
            domain: slug.into(),
            template: None,
            params: None,
            statement: None,
            status: Some("historical".into()),
            tradition: None,
            source_ref: None,
            source_kind: None,
            pramana: None,
            confidence: None,
            id: Some(claim_id.clone()),
        },
    )
    .await
    .expect("active → historical should succeed");
    assert_eq!(updated2.claim.as_ref().unwrap().status, "historical");

    // historical → anything: NOT allowed
    let err = tools::claim::handle(
        &pool,
        tools::ClaimArgs {
            action: "update".into(),
            domain: slug.into(),
            template: None,
            params: None,
            statement: None,
            status: Some("active".into()),
            tradition: None,
            source_ref: None,
            source_kind: None,
            pramana: None,
            confidence: None,
            id: Some(claim_id.clone()),
        },
    )
    .await
    .expect_err("historical → active should fail");
    let msg = err.to_string();
    assert!(msg.contains("historical"), "error should mention current status: {msg}");

    // Create another claim and test active → proposed: NOT allowed
    let created2 = tools::claim::handle(
        &pool,
        tools::ClaimArgs {
            action: "create".into(),
            domain: slug.into(),
            template: Some("rule".into()),
            params: Some(json!({"x": 1})),
            statement: Some("another claim".into()),
            status: Some("active".into()),
            tradition: None,
            source_ref: None,
            source_kind: None,
            pramana: None,
            confidence: None,
            id: None,
        },
    )
    .await
    .expect("create second claim");
    let claim2_id = created2.claim.unwrap().id.to_string();

    let err2 = tools::claim::handle(
        &pool,
        tools::ClaimArgs {
            action: "update".into(),
            domain: slug.into(),
            template: None,
            params: None,
            statement: None,
            status: Some("proposed".into()),
            tradition: None,
            source_ref: None,
            source_kind: None,
            pramana: None,
            confidence: None,
            id: Some(claim2_id),
        },
    )
    .await
    .expect_err("active → proposed should fail");
    let msg2 = err2.to_string();
    assert!(msg2.contains("active"), "error should mention current status: {msg2}");

    cleanup(&pool, slug).await;
}

#[tokio::test]
async fn relation_create_and_list() {
    let pool = test_pool().await;
    let slug = "test-relations";

    cleanup(&pool, slug).await;

    // Setup: domain with entity_kinds, relation_kinds, and entities
    let payload = json!({
        "domain": { "slug": slug, "title": "Relation Test" },
        "entity_kinds": [
            { "slug": "graha", "schema": null },
            { "slug": "rashi", "schema": null }
        ],
        "relation_kinds": [
            { "slug": "rules", "src_kind": "graha", "dst_kind": "rashi" }
        ],
        "entities": [
            { "kind": "graha", "name": "Sūrya", "attrs": {} },
            { "kind": "rashi", "name": "Siṃha", "attrs": {} },
            { "kind": "rashi", "name": "Meṣa", "attrs": {} }
        ]
    });
    tools::load::handle(&pool, tools::LoadArgs { payload })
        .await
        .expect("setup load");

    // Create a relation
    let created = tools::relation::handle(
        &pool,
        tools::RelationArgs {
            action: "create".into(),
            domain: slug.into(),
            kind: Some("rules".into()),
            src_entity: Some("Sūrya".into()),
            dst_entity: Some("Siṃha".into()),
            src_domain: None,
            dst_domain: None,
            attrs: None,
            id: None,
            entity: None,
            entity_domain: None,
        },
    )
    .await
    .expect("create relation");
    assert_eq!(created.action, "created");
    let rel = created.relation.unwrap();
    let rel_id = rel.id.to_string();

    // Get by ID
    let got = tools::relation::handle(
        &pool,
        tools::RelationArgs {
            action: "get".into(),
            domain: slug.into(),
            kind: None,
            src_entity: None,
            dst_entity: None,
            src_domain: None,
            dst_domain: None,
            attrs: None,
            id: Some(rel_id),
            entity: None,
            entity_domain: None,
        },
    )
    .await
    .expect("get relation");
    assert_eq!(got.action, "found");
    assert_eq!(got.relation.unwrap().id, rel.id);

    // List relations for Sūrya
    let listed = tools::relation::handle(
        &pool,
        tools::RelationArgs {
            action: "list".into(),
            domain: slug.into(),
            kind: None,
            src_entity: None,
            dst_entity: None,
            src_domain: None,
            dst_domain: None,
            attrs: None,
            id: None,
            entity: Some("Sūrya".into()),
            entity_domain: None,
        },
    )
    .await
    .expect("list relations");
    assert_eq!(listed.action, "listed");
    let rels = listed.relations.unwrap();
    assert_eq!(rels.len(), 1);

    cleanup(&pool, slug).await;
}

#[tokio::test]
async fn cross_domain_relation() {
    let pool = test_pool().await;
    let slug_a = "test-xdomain-a";
    let slug_b = "test-xdomain-b";

    cleanup(&pool, slug_a).await;
    cleanup(&pool, slug_b).await;

    // Domain A: has a "concept" entity kind and entity "dharma"
    let payload_a = json!({
        "domain": { "slug": slug_a, "title": "Domain A" },
        "entity_kinds": [{ "slug": "concept", "schema": null }],
        "entities": [{ "kind": "concept", "name": "dharma", "attrs": {} }]
    });
    tools::load::handle(&pool, tools::LoadArgs { payload: payload_a })
        .await
        .expect("load domain A");

    // Domain B: has a "text" entity kind, entity "Gītā", and a relation_kind "discusses"
    let payload_b = json!({
        "domain": { "slug": slug_b, "title": "Domain B" },
        "entity_kinds": [{ "slug": "text", "schema": null }],
        "relation_kinds": [{ "slug": "discusses", "src_kind": "text", "dst_kind": null }],
        "entities": [{ "kind": "text", "name": "Gītā", "attrs": {} }]
    });
    tools::load::handle(&pool, tools::LoadArgs { payload: payload_b })
        .await
        .expect("load domain B");

    // Create cross-domain relation: Gītā (domain B) discusses dharma (domain A)
    let created = tools::relation::handle(
        &pool,
        tools::RelationArgs {
            action: "create".into(),
            domain: slug_b.into(),
            kind: Some("discusses".into()),
            src_entity: Some("Gītā".into()),
            dst_entity: Some("dharma".into()),
            src_domain: Some(slug_b.into()),
            dst_domain: Some(slug_a.into()),
            attrs: None,
            id: None,
            entity: None,
            entity_domain: None,
        },
    )
    .await
    .expect("cross-domain relation should succeed");
    assert_eq!(created.action, "created");
    let rel = created.relation.unwrap();

    // Verify src and dst are from different domains
    let src_entity = sqlx::query_as::<_, vidya::db::EntityRow>(
        "SELECT * FROM entities WHERE id = $1",
    )
    .bind(rel.src_entity_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let dst_entity = sqlx::query_as::<_, vidya::db::EntityRow>(
        "SELECT * FROM entities WHERE id = $1",
    )
    .bind(rel.dst_entity_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_ne!(
        src_entity.domain_id, dst_entity.domain_id,
        "entities should be from different domains"
    );

    // List by entity in the other domain
    let listed = tools::relation::handle(
        &pool,
        tools::RelationArgs {
            action: "list".into(),
            domain: slug_b.into(),
            kind: None,
            src_entity: None,
            dst_entity: None,
            src_domain: None,
            dst_domain: None,
            attrs: None,
            id: None,
            entity: Some("dharma".into()),
            entity_domain: Some(slug_a.into()),
        },
    )
    .await
    .expect("list cross-domain relations");
    assert_eq!(listed.relations.unwrap().len(), 1);

    cleanup(&pool, slug_b).await;
    cleanup(&pool, slug_a).await;
}

#[tokio::test]
#[serial]
async fn jyotish_query_only() {
    let pool = test_pool().await;
    let slug = "jyotish";
    let source_slugs = &["bphs", "phala-dipika", "jataka-parijata", "saravali"];

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, source_slugs).await;

    load_seed_file(&pool, Path::new("seeds/jyotish.json")).await;

    // Query entities by kind "graha" — should get 9 planets
    let grahas = tools::entity::handle(
        &pool,
        tools::EntityArgs {
            action: "list".into(),
            domain: slug.into(),
            kind: Some("graha".into()),
            name: None,
            attrs: None,
        },
    )
    .await
    .expect("list grahas");
    assert_eq!(
        grahas.entities.as_ref().unwrap().len(),
        9,
        "should have 9 grahas (navagraha)"
    );

    // Query claims by template "dignity"
    let dignities = tools::claim::handle(
        &pool,
        tools::ClaimArgs {
            action: "list".into(),
            domain: slug.into(),
            template: Some("dignity".into()),
            params: None,
            statement: None,
            status: None,
            tradition: None,
            source_ref: None,
            source_kind: None,
            pramana: None,
            confidence: None,
            id: None,
        },
    )
    .await
    .expect("list dignity claims");
    assert!(
        dignities.claims.as_ref().unwrap().len() > 0,
        "should have dignity claims"
    );

    // Query entity "Sūrya"
    let surya = tools::entity::handle(
        &pool,
        tools::EntityArgs {
            action: "get".into(),
            domain: slug.into(),
            kind: None,
            name: Some("Sūrya".into()),
            attrs: None,
        },
    )
    .await
    .expect("get Sūrya");
    assert_eq!(surya.entity.unwrap().name, "Sūrya");

    // vidya_derive for jyotish should return a clean error (no engine strategy for "dignity")
    let err = tools::derive::handle(
        &pool,
        tools::DeriveArgs {
            domain: slug.into(),
            operation: "dignity".into(),
            input: json!({"graha": "Sūrya", "rashi": "Meṣa"}),
        },
    )
    .await
    .expect_err("derive should fail for unsupported operation");
    let msg = err.to_string();
    assert!(
        msg.contains("supported operations") || msg.contains("operation"),
        "should give clean error about unsupported operation: {msg}"
    );

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, source_slugs).await;
}

#[tokio::test]
#[serial]
async fn full_integration_both_seeds() {
    let pool = test_pool().await;

    let vya_sources = &["ashtadhyayi", "shiva-sutras"];
    let jyo_sources = &["bphs", "phala-dipika", "jataka-parijata", "saravali"];

    cleanup(&pool, "vyakarana").await;
    cleanup(&pool, "jyotish").await;
    cleanup_sources(&pool, vya_sources).await;
    cleanup_sources(&pool, jyo_sources).await;

    // Load both seeds
    let vya = load_seed_file(&pool, Path::new("seeds/vyakarana.json")).await;
    let jyo = load_seed_file(&pool, Path::new("seeds/jyotish.json")).await;

    // Verify entity counts per domain
    assert_eq!(vya.entities, 44);
    assert!(jyo.entities > 0);

    // Verify claim counts per domain
    assert_eq!(vya.claims, 32);
    assert!(jyo.claims > 0);

    // Verify relations exist in jyotish
    assert!(jyo.relations > 0, "jyotish should have relations");

    // Verify assertions have correct tradition + source
    let surya_query = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: "jyotish".into(),
            entity: Some("Sūrya".into()),
            entity_kind: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            include_provenance: true,
        },
    )
    .await
    .expect("query Sūrya");

    let entity_ctx = surya_query.entity.expect("should have entity context");
    assert_eq!(entity_ctx.entity.name, "Sūrya");
    assert!(!entity_ctx.claims.is_empty(), "Sūrya should have claims");
    assert!(!entity_ctx.relations.is_empty(), "Sūrya should have relations");

    // Verify at least one assertion has provenance
    let has_provenance = entity_ctx.claims.iter().any(|c| {
        c.assertions
            .as_ref()
            .map_or(false, |a| !a.is_empty())
    });
    assert!(has_provenance, "at least one claim should have provenance assertions");

    // Verify traditions are hierarchical (kāśikā has parent pāṇini)
    let vya_domain = vidya::db::get_domain_by_slug(&pool, "vyakarana")
        .await
        .unwrap()
        .expect("vyakarana domain should exist");

    let kasika = sqlx::query_as::<_, vidya::db::TraditionRow>(
        "SELECT * FROM traditions WHERE domain_id = $1 AND name = 'kāśikā'",
    )
    .bind(vya_domain.id)
    .fetch_optional(&pool)
    .await
    .unwrap();
    let kasika = kasika.expect("kāśikā tradition should exist");
    assert!(kasika.parent_id.is_some(), "kāśikā should have a parent");

    let panini = sqlx::query_as::<_, vidya::db::TraditionRow>(
        "SELECT * FROM traditions WHERE domain_id = $1 AND name = 'pāṇini'",
    )
    .bind(vya_domain.id)
    .fetch_optional(&pool)
    .await
    .unwrap();
    let panini = panini.expect("pāṇini tradition should exist");
    assert_eq!(
        kasika.parent_id.unwrap(),
        panini.id,
        "kāśikā's parent should be pāṇini"
    );

    // Verify sources have reliability scores
    let ashtadhyayi = sqlx::query_as::<_, vidya::db::SourceRow>(
        "SELECT * FROM sources WHERE slug = 'ashtadhyayi'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ashtadhyayi.reliability, Some(1.0));

    cleanup(&pool, "jyotish").await;
    cleanup(&pool, "vyakarana").await;
    cleanup_sources(&pool, vya_sources).await;
    cleanup_sources(&pool, jyo_sources).await;
}
