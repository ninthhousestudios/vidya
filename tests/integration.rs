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
        "DELETE FROM derivations WHERE conclusion_claim_id IN \
         (SELECT id FROM claims WHERE domain_id = (SELECT id FROM domains WHERE slug = $1)) \
         OR premise_claim_id IN \
         (SELECT id FROM claims WHERE domain_id = (SELECT id FROM domains WHERE slug = $1))",
    )
    .bind(domain_slug)
    .bind(domain_slug)
    .execute(pool)
    .await;
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

    // Reject wrong entity kind: "rules" is graha→rashi, try rashi→rashi
    let err = tools::relation::handle(
        &pool,
        tools::RelationArgs {
            action: "create".into(),
            domain: slug.into(),
            kind: Some("rules".into()),
            src_entity: Some("Siṃha".into()),
            dst_entity: Some("Meṣa".into()),
            src_domain: None,
            dst_domain: None,
            attrs: None,
            id: None,
            entity: None,
            entity_domain: None,
        },
    )
    .await
    .expect_err("should reject wrong src entity kind");
    let msg = err.to_string();
    assert!(
        msg.contains("src_entity") || msg.contains("declared kind"),
        "error should mention kind mismatch: {msg}"
    );

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
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
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

// --- vidya_query expansion tests ---

#[tokio::test]
async fn query_entities_by_kind() {
    let pool = test_pool().await;
    let slug = "test-query-kind";

    cleanup(&pool, slug).await;

    let payload = json!({
        "domain": { "slug": slug, "title": "Query Kind Test" },
        "entity_kinds": [
            { "slug": "vowel", "schema": null },
            { "slug": "consonant", "schema": null }
        ],
        "entities": [
            { "kind": "vowel", "name": "a", "attrs": { "class": "short" } },
            { "kind": "vowel", "name": "ā", "attrs": { "class": "long" } },
            { "kind": "vowel", "name": "i", "attrs": { "class": "short" } },
            { "kind": "consonant", "name": "ka", "attrs": {} },
            { "kind": "consonant", "name": "kha", "attrs": {} }
        ]
    });
    tools::load::handle(&pool, tools::LoadArgs { payload })
        .await
        .expect("setup load");

    let result = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: slug.into(),
            entity: None,
            entity_kind: Some("vowel".into()),
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("query by kind");

    let entities = result.entities.expect("should return entities list");
    assert_eq!(entities.len(), 3, "should find 3 vowels");
    assert!(entities.iter().all(|e| e.name == "a" || e.name == "ā" || e.name == "i"));

    // Verify single-entity mode still works
    assert!(result.entity.is_none());
    assert!(result.claims.is_none());

    cleanup(&pool, slug).await;
}

#[tokio::test]
async fn query_entities_by_name_pattern() {
    let pool = test_pool().await;
    let slug = "test-query-pattern";

    cleanup(&pool, slug).await;

    let payload = json!({
        "domain": { "slug": slug, "title": "Query Pattern Test" },
        "entity_kinds": [
            { "slug": "varna", "schema": null }
        ],
        "entities": [
            { "kind": "varna", "name": "ka", "attrs": {} },
            { "kind": "varna", "name": "kha", "attrs": {} },
            { "kind": "varna", "name": "ga", "attrs": {} },
            { "kind": "varna", "name": "gha", "attrs": {} }
        ]
    });
    tools::load::handle(&pool, tools::LoadArgs { payload })
        .await
        .expect("setup load");

    // Substring match: "kh" should find "kha"
    let result = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: slug.into(),
            entity: None,
            entity_kind: None,
            name_pattern: Some("kh".into()),
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("query by pattern");

    let entities = result.entities.expect("should return entities list");
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].name, "kha");

    // Pattern "a" matches "ka", "kha", "ga", "gha" (all contain 'a')
    let result2 = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: slug.into(),
            entity: None,
            entity_kind: None,
            name_pattern: Some("a".into()),
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("query by broad pattern");

    let entities2 = result2.entities.expect("should return entities list");
    assert_eq!(entities2.len(), 4);

    cleanup(&pool, slug).await;
}

#[tokio::test]
async fn query_entities_by_attrs() {
    let pool = test_pool().await;
    let slug = "test-query-attrs";

    cleanup(&pool, slug).await;

    let payload = json!({
        "domain": { "slug": slug, "title": "Query Attrs Test" },
        "entity_kinds": [
            { "slug": "vowel", "schema": null }
        ],
        "entities": [
            { "kind": "vowel", "name": "a", "attrs": { "class": "short", "type": "simple" } },
            { "kind": "vowel", "name": "ā", "attrs": { "class": "long", "type": "simple" } },
            { "kind": "vowel", "name": "e", "attrs": { "class": "long", "type": "guṇa" } },
            { "kind": "vowel", "name": "ai", "attrs": { "class": "long", "type": "vṛddhi" } }
        ]
    });
    tools::load::handle(&pool, tools::LoadArgs { payload })
        .await
        .expect("setup load");

    // Filter by single attr: class=short
    let result = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: slug.into(),
            entity: None,
            entity_kind: Some("vowel".into()),
            name_pattern: None,
            attrs: Some(json!({ "class": "short" })),
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("query by attrs");

    let entities = result.entities.expect("should return entities list");
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].name, "a");

    // Filter by multiple attrs: class=long AND type=guṇa
    let result2 = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: slug.into(),
            entity: None,
            entity_kind: Some("vowel".into()),
            name_pattern: None,
            attrs: Some(json!({ "class": "long", "type": "guṇa" })),
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("query by multiple attrs");

    let entities2 = result2.entities.expect("should return entities list");
    assert_eq!(entities2.len(), 1);
    assert_eq!(entities2[0].name, "e");

    cleanup(&pool, slug).await;
}

#[tokio::test]
async fn query_relation_kind_filter() {
    let pool = test_pool().await;
    let slug = "test-query-relkind";

    cleanup(&pool, slug).await;

    let payload = json!({
        "domain": { "slug": slug, "title": "Relation Kind Filter Test" },
        "entity_kinds": [
            { "slug": "graha", "schema": null },
            { "slug": "rashi", "schema": null }
        ],
        "relation_kinds": [
            { "slug": "rules", "src_kind": "graha", "dst_kind": "rashi" },
            { "slug": "exalted_in", "src_kind": "graha", "dst_kind": "rashi" }
        ],
        "entities": [
            { "kind": "graha", "name": "Sūrya", "attrs": {} },
            { "kind": "rashi", "name": "Siṃha", "attrs": {} },
            { "kind": "rashi", "name": "Meṣa", "attrs": {} }
        ],
        "relations": [
            { "kind": "rules", "src": "Sūrya", "dst": "Siṃha" },
            { "kind": "exalted_in", "src": "Sūrya", "dst": "Meṣa" }
        ]
    });
    tools::load::handle(&pool, tools::LoadArgs { payload })
        .await
        .expect("setup load");

    // No filter: both relations
    let result = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: slug.into(),
            entity: Some("Sūrya".into()),
            entity_kind: None,
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("query all relations");

    let ctx = result.entity.as_ref().expect("should have entity context");
    assert_eq!(ctx.relations.len(), 2);

    // Filter by "exalted_in": only one relation
    let result2 = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: slug.into(),
            entity: Some("Sūrya".into()),
            entity_kind: None,
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: Some("exalted_in".into()),
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("query filtered relations");

    let ctx2 = result2.entity.as_ref().expect("should have entity context");
    assert_eq!(ctx2.relations.len(), 1);
    assert_eq!(ctx2.relations[0].kind_slug, "exalted_in");
    assert_eq!(ctx2.relations[0].other_entity_name, "Meṣa");

    cleanup(&pool, slug).await;
}

#[tokio::test]
async fn query_relation_traverse_depth() {
    let pool = test_pool().await;
    let slug = "test-query-traverse";

    cleanup(&pool, slug).await;

    // Chain: A -[links_to]-> B -[links_to]-> C -[links_to]-> D
    let payload = json!({
        "domain": { "slug": slug, "title": "Traverse Test" },
        "entity_kinds": [
            { "slug": "node", "schema": null }
        ],
        "relation_kinds": [
            { "slug": "links_to", "src_kind": "node", "dst_kind": "node" }
        ],
        "entities": [
            { "kind": "node", "name": "A", "attrs": {} },
            { "kind": "node", "name": "B", "attrs": {} },
            { "kind": "node", "name": "C", "attrs": {} },
            { "kind": "node", "name": "D", "attrs": {} }
        ],
        "relations": [
            { "kind": "links_to", "src": "A", "dst": "B" },
            { "kind": "links_to", "src": "B", "dst": "C" },
            { "kind": "links_to", "src": "C", "dst": "D" }
        ]
    });
    tools::load::handle(&pool, tools::LoadArgs { payload })
        .await
        .expect("setup load");

    // depth=1: only B
    let result = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: slug.into(),
            entity: Some("A".into()),
            entity_kind: None,
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("depth=1 query");

    let ctx = result.entity.as_ref().unwrap();
    assert_eq!(ctx.relations.len(), 1);
    assert_eq!(ctx.relations[0].other_entity_name, "B");
    assert_eq!(ctx.relations[0].depth, 1);

    // depth=2: B and C
    let result2 = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: slug.into(),
            entity: Some("A".into()),
            entity_kind: None,
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 2,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("depth=2 query");

    let ctx2 = result2.entity.as_ref().unwrap();
    assert_eq!(ctx2.relations.len(), 2);
    let depth1: Vec<_> = ctx2.relations.iter().filter(|r| r.depth == 1).collect();
    let depth2: Vec<_> = ctx2.relations.iter().filter(|r| r.depth == 2).collect();
    assert_eq!(depth1.len(), 1);
    assert_eq!(depth2.len(), 1);

    // depth=3: B, C, and D
    let result3 = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: slug.into(),
            entity: Some("A".into()),
            entity_kind: None,
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 3,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("depth=3 query");

    let ctx3 = result3.entity.as_ref().unwrap();
    assert_eq!(ctx3.relations.len(), 3);
    let depth3: Vec<_> = ctx3.relations.iter().filter(|r| r.depth == 3).collect();
    assert_eq!(depth3.len(), 1);

    cleanup(&pool, slug).await;
}

#[tokio::test]
async fn query_claim_provenance_with_derivation() {
    let pool = test_pool().await;
    let slug = "test-query-provenance";

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, &["test-src-prov"]).await;

    let payload = json!({
        "domain": { "slug": slug, "title": "Provenance Test" },
        "entity_kinds": [],
        "claim_templates": [
            { "slug": "rule", "param_schema": {} }
        ],
        "traditions": [{ "name": "test-tradition" }],
        "sources": [{ "slug": "test-src-prov", "kind": "text", "reference": "Test Source", "reliability": 1.0 }],
        "claims": [
            {
                "template": "rule",
                "params": { "name": "premise-1" },
                "statement": "First premise",
                "tradition": "test-tradition",
                "source": "test-src-prov",
                "pramana": "pratyaksha",
                "confidence": 1.0
            },
            {
                "template": "rule",
                "params": { "name": "premise-2" },
                "statement": "Second premise",
                "tradition": "test-tradition",
                "source": "test-src-prov",
                "pramana": "shabda",
                "confidence": 0.9
            }
        ]
    });
    tools::load::handle(&pool, tools::LoadArgs { payload })
        .await
        .expect("setup load");

    // Create a conclusion claim via vidya_claim
    let conclusion = tools::claim::handle(
        &pool,
        tools::ClaimArgs {
            action: "create".into(),
            domain: slug.into(),
            template: Some("rule".into()),
            params: Some(json!({ "name": "conclusion" })),
            statement: Some("Derived conclusion".into()),
            status: Some("active".into()),
            tradition: Some("test-tradition".into()),
            source_ref: Some("test-src-prov".into()),
            source_kind: None,
            pramana: Some("anumana".into()),
            confidence: Some(0.8),
            id: None,
        },
    )
    .await
    .expect("create conclusion claim");
    let conclusion_id = conclusion.claim.as_ref().unwrap().id;

    // Get the premise claim IDs
    let premise1 = sqlx::query_as::<_, vidya::db::ClaimRow>(
        "SELECT * FROM claims WHERE domain_id = (SELECT id FROM domains WHERE slug = $1) \
         AND params @> '{\"name\": \"premise-1\"}'",
    )
    .bind(slug)
    .fetch_one(&pool)
    .await
    .unwrap();

    let premise2 = sqlx::query_as::<_, vidya::db::ClaimRow>(
        "SELECT * FROM claims WHERE domain_id = (SELECT id FROM domains WHERE slug = $1) \
         AND params @> '{\"name\": \"premise-2\"}'",
    )
    .bind(slug)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Insert derivation links
    vidya::db::insert_derivation(&pool, conclusion_id, premise1.id, 1)
        .await
        .expect("insert derivation 1");
    vidya::db::insert_derivation(&pool, conclusion_id, premise2.id, 2)
        .await
        .expect("insert derivation 2");

    // Query provenance for the conclusion
    let result = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: slug.into(),
            entity: None,
            entity_kind: None,
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: Some(conclusion_id.to_string()),
            include_provenance: true,
        },
    )
    .await
    .expect("query provenance");

    let prov = result.provenance.expect("should have provenance");
    assert_eq!(prov.claim.id, conclusion_id);
    assert_eq!(prov.template_slug, "rule");
    assert_eq!(prov.assertions.len(), 1);
    assert_eq!(prov.assertions[0].tradition_name, "test-tradition");
    assert_eq!(prov.derivation_chain.len(), 2);
    assert_eq!(prov.derivation_chain[0].step_order, 1);
    assert_eq!(prov.derivation_chain[0].premise.statement, "First premise");
    assert_eq!(prov.derivation_chain[1].step_order, 2);
    assert_eq!(prov.derivation_chain[1].premise.statement, "Second premise");

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, &["test-src-prov"]).await;
}

#[tokio::test]
async fn query_cross_entity_predicate() {
    let pool = test_pool().await;
    let slug = "test-query-xpred";

    cleanup(&pool, slug).await;

    let payload = json!({
        "domain": { "slug": slug, "title": "Cross-Entity Predicate Test" },
        "entity_kinds": [
            { "slug": "varna", "schema": null }
        ],
        "claim_templates": [
            { "slug": "sound_classification", "param_schema": {} }
        ],
        "entities": [
            { "kind": "varna", "name": "a", "attrs": {} },
            { "kind": "varna", "name": "ā", "attrs": {} },
            { "kind": "varna", "name": "e", "attrs": {} },
            { "kind": "varna", "name": "ka", "attrs": {} }
        ],
        "claims": [
            { "template": "sound_classification", "params": { "varna": "a", "classification": "guṇa" }, "statement": "a is guṇa" },
            { "template": "sound_classification", "params": { "varna": "ā", "classification": "vṛddhi" }, "statement": "ā is vṛddhi" },
            { "template": "sound_classification", "params": { "varna": "e", "classification": "guṇa" }, "statement": "e is guṇa" },
            { "template": "sound_classification", "params": { "varna": "ka", "classification": "sparśa" }, "statement": "ka is sparśa" }
        ]
    });
    tools::load::handle(&pool, tools::LoadArgs { payload })
        .await
        .expect("setup load");

    // Cross-entity: all varnas where sound_classification has classification=guṇa
    let result = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: slug.into(),
            entity: None,
            entity_kind: Some("varna".into()),
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: Some("sound_classification".into()),
            claim_params: Some(json!({ "classification": "guṇa" })),
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("cross-entity predicate query");

    let entities = result.entities.expect("should return entities list");
    assert_eq!(entities.len(), 2, "should find 2 guṇa varnas");
    let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"a"));
    assert!(names.contains(&"e"));

    cleanup(&pool, slug).await;
}

#[tokio::test]
#[serial]
async fn query_all_modes_against_seeds() {
    let pool = test_pool().await;

    let vya_sources = &["ashtadhyayi", "shiva-sutras"];
    let jyo_sources = &["bphs", "phala-dipika", "jataka-parijata", "saravali"];

    cleanup(&pool, "vyakarana").await;
    cleanup(&pool, "jyotish").await;
    cleanup_sources(&pool, vya_sources).await;
    cleanup_sources(&pool, jyo_sources).await;

    load_seed_file(&pool, Path::new("seeds/vyakarana.json")).await;
    load_seed_file(&pool, Path::new("seeds/jyotish.json")).await;

    // --- AC 1: entities by kind ---
    let vowels = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: "vyakarana".into(),
            entity: None,
            entity_kind: Some("varna".into()),
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("AC1: entities by kind");
    let varna_entities = vowels.entities.unwrap();
    assert!(varna_entities.len() > 10, "should have many varnas");

    // --- AC 2: entities by name pattern ---
    let pattern_result = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: "jyotish".into(),
            entity: None,
            entity_kind: None,
            name_pattern: Some("Sū".into()),
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("AC2: name pattern");
    let pattern_entities = pattern_result.entities.unwrap();
    assert!(pattern_entities.iter().any(|e| e.name == "Sūrya"));

    // --- AC 3: entities by attrs ---
    let short_vowels = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: "vyakarana".into(),
            entity: None,
            entity_kind: Some("varna".into()),
            name_pattern: None,
            attrs: Some(json!({ "class": "vowel", "short": true })),
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("AC3: attrs filter");
    let sv = short_vowels.entities.unwrap();
    assert!(sv.len() >= 4, "should have short vowels (a, i, u, ṛ, ḷ)");

    // --- AC 4: claims for entity ---
    let surya = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: "jyotish".into(),
            entity: Some("Sūrya".into()),
            entity_kind: None,
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: true,
        },
    )
    .await
    .expect("AC4: claims for entity");
    let surya_ctx = surya.entity.unwrap();
    assert!(!surya_ctx.claims.is_empty(), "Sūrya should have claims");

    // --- AC 5: claims filtered by tradition ---
    let panini_claims = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: "vyakarana".into(),
            entity: None,
            entity_kind: None,
            name_pattern: None,
            attrs: None,
            tradition: Some("pāṇini".into()),
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: true,
        },
    )
    .await
    .expect("AC5: tradition filter");
    let pc = panini_claims.claims.unwrap();
    assert!(!pc.is_empty(), "should have pāṇini tradition claims");

    // --- AC 6: claims filtered by template ---
    let sandhi_claims = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: "vyakarana".into(),
            entity: None,
            entity_kind: None,
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: Some("sandhi_rule".into()),
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("AC6: template filter");
    let sc = sandhi_claims.claims.unwrap();
    assert!(sc.len() >= 10, "should have sandhi rules");

    // --- AC 7: relation traversal by kind, depth=1 ---
    let surya_exalt = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: "jyotish".into(),
            entity: Some("Sūrya".into()),
            entity_kind: None,
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: Some("exalted_in".into()),
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("AC7: relation kind filter");
    let exalt_ctx = surya_exalt.entity.unwrap();
    assert_eq!(exalt_ctx.relations.len(), 1, "Sūrya exalted in one rashi");
    assert_eq!(exalt_ctx.relations[0].other_entity_name, "Meṣa");

    // --- AC 9: provenance assertions ---
    let surya_prov = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: "jyotish".into(),
            entity: Some("Sūrya".into()),
            entity_kind: None,
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: true,
        },
    )
    .await
    .expect("AC9: provenance");
    let sp_ctx = surya_prov.entity.unwrap();
    let has_assertions = sp_ctx.claims.iter().any(|c| {
        c.assertions.as_ref().map_or(false, |a| !a.is_empty())
    });
    assert!(has_assertions, "Sūrya claims should have assertion provenance");

    // --- AC 11: cross-entity predicate against jyotish ---
    let exalted_grahas = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: "jyotish".into(),
            entity: None,
            entity_kind: Some("graha".into()),
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: Some("dignity".into()),
            claim_params: Some(json!({ "dignity_type": "exaltation" })),
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("AC11: cross-entity predicate");
    let eg = exalted_grahas.entities.unwrap();
    assert!(eg.len() >= 7, "most grahas have exaltation");
    let graha_names: Vec<&str> = eg.iter().map(|e| e.name.as_str()).collect();
    assert!(graha_names.contains(&"Sūrya"), "Sūrya should be exalted");

    cleanup(&pool, "jyotish").await;
    cleanup(&pool, "vyakarana").await;
    cleanup_sources(&pool, vya_sources).await;
    cleanup_sources(&pool, jyo_sources).await;
}

#[tokio::test]
async fn cross_entity_predicate_no_false_match_short_names() {
    let pool = test_pool().await;
    let slug = "test-xpred-false";

    cleanup(&pool, slug).await;

    let payload = json!({
        "domain": { "slug": slug, "title": "Cross-Entity False Match Test" },
        "entity_kinds": [
            { "slug": "varna", "schema": null }
        ],
        "claim_templates": [
            { "slug": "classification", "param_schema": {} }
        ],
        "entities": [
            { "kind": "varna", "name": "a", "attrs": {} },
            { "kind": "varna", "name": "ka", "attrs": {} }
        ],
        "claims": [
            { "template": "classification", "params": { "varna": "ka", "classification": "sparśa" }, "statement": "ka is sparśa" }
        ]
    });
    tools::load::handle(&pool, tools::LoadArgs { payload })
        .await
        .expect("setup load");

    // "a" appears in JSON text ("sparśa", "classification", "varna") but is NOT a param value.
    // Only "ka" is a param value, so only "ka" should match.
    let result = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: slug.into(),
            entity: None,
            entity_kind: Some("varna".into()),
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: Some("classification".into()),
            claim_params: Some(json!({ "classification": "sparśa" })),
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await
    .expect("cross-entity predicate");

    let entities = result.entities.expect("should return entities");
    let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["ka"], "only 'ka' should match, not 'a'");

    cleanup(&pool, slug).await;
}

#[tokio::test]
async fn attrs_filter_rejects_non_object() {
    let pool = test_pool().await;
    let slug = "test-attrs-noobj";

    cleanup(&pool, slug).await;

    let payload = json!({
        "domain": { "slug": slug, "title": "Attrs Non-Object Test" },
        "entity_kinds": [
            { "slug": "node", "schema": null }
        ],
        "entities": [
            { "kind": "node", "name": "x", "attrs": {} }
        ]
    });
    tools::load::handle(&pool, tools::LoadArgs { payload })
        .await
        .expect("setup load");

    let result = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: slug.into(),
            entity: None,
            entity_kind: Some("node".into()),
            name_pattern: None,
            attrs: Some(json!("not-an-object")),
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: None,
            include_provenance: false,
        },
    )
    .await;

    assert!(result.is_err(), "non-object attrs should return error");

    cleanup(&pool, slug).await;
}

#[tokio::test]
async fn derivation_chain_multi_level() {
    let pool = test_pool().await;
    let slug = "test-deriv-chain";

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, &["test-src-chain"]).await;

    let payload = json!({
        "domain": { "slug": slug, "title": "Derivation Chain Test" },
        "entity_kinds": [],
        "claim_templates": [
            { "slug": "rule", "param_schema": {} }
        ],
        "traditions": [{ "name": "test-tradition" }],
        "sources": [{ "slug": "test-src-chain", "kind": "text", "reference": "Test Source", "reliability": 1.0 }],
        "claims": [
            {
                "template": "rule",
                "params": { "name": "source-fact" },
                "statement": "Source fact (no derivation)",
                "tradition": "test-tradition",
                "source": "test-src-chain",
                "pramana": "pratyaksha",
                "confidence": 1.0
            }
        ]
    });
    tools::load::handle(&pool, tools::LoadArgs { payload })
        .await
        .expect("setup load");

    let source_claim = sqlx::query_as::<_, vidya::db::ClaimRow>(
        "SELECT * FROM claims WHERE domain_id = (SELECT id FROM domains WHERE slug = $1) \
         AND params @> '{\"name\": \"source-fact\"}'",
    )
    .bind(slug)
    .fetch_one(&pool)
    .await
    .unwrap();

    let intermediate = tools::claim::handle(
        &pool,
        tools::ClaimArgs {
            action: "create".into(),
            domain: slug.into(),
            template: Some("rule".into()),
            params: Some(json!({ "name": "intermediate" })),
            statement: Some("Intermediate derivation".into()),
            status: Some("active".into()),
            tradition: Some("test-tradition".into()),
            source_ref: Some("test-src-chain".into()),
            source_kind: None,
            pramana: Some("anumana".into()),
            confidence: Some(0.9),
            id: None,
        },
    )
    .await
    .expect("create intermediate claim");
    let intermediate_id = intermediate.claim.as_ref().unwrap().id;

    let conclusion = tools::claim::handle(
        &pool,
        tools::ClaimArgs {
            action: "create".into(),
            domain: slug.into(),
            template: Some("rule".into()),
            params: Some(json!({ "name": "conclusion" })),
            statement: Some("Final conclusion".into()),
            status: Some("active".into()),
            tradition: Some("test-tradition".into()),
            source_ref: Some("test-src-chain".into()),
            source_kind: None,
            pramana: Some("anumana".into()),
            confidence: Some(0.8),
            id: None,
        },
    )
    .await
    .expect("create conclusion claim");
    let conclusion_id = conclusion.claim.as_ref().unwrap().id;

    // Wire: conclusion <- intermediate <- source
    vidya::db::insert_derivation(&pool, conclusion_id, intermediate_id, 1)
        .await
        .expect("derivation: conclusion <- intermediate");
    vidya::db::insert_derivation(&pool, intermediate_id, source_claim.id, 1)
        .await
        .expect("derivation: intermediate <- source");

    let result = tools::query::handle(
        &pool,
        tools::QueryArgs {
            domain: slug.into(),
            entity: None,
            entity_kind: None,
            name_pattern: None,
            attrs: None,
            tradition: None,
            pramana: None,
            claim_template: None,
            claim_params: None,
            relation_kind: None,
            traverse_depth: 1,
            claim_id: Some(conclusion_id.to_string()),
            include_provenance: true,
        },
    )
    .await
    .expect("query provenance");

    let prov = result.provenance.expect("should have provenance");
    assert_eq!(prov.claim.id, conclusion_id);
    assert_eq!(
        prov.derivation_chain.len(),
        2,
        "should trace through intermediate to source: got {:?}",
        prov.derivation_chain.iter().map(|d| d.premise.statement.as_str()).collect::<Vec<_>>()
    );

    let statements: Vec<&str> = prov.derivation_chain.iter().map(|d| d.premise.statement.as_str()).collect();
    assert!(statements.contains(&"Intermediate derivation"), "should include intermediate");
    assert!(statements.contains(&"Source fact (no derivation)"), "should include source fact");

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, &["test-src-chain"]).await;
}

// --- Engine strategy + sandhi derivation tests ---

#[tokio::test]
#[serial]
async fn derive_sandhi_all_ten_cases() {
    let pool = test_pool().await;
    let slug = "vyakarana";
    let source_slugs = &["ashtadhyayi", "shiva-sutras"];

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, source_slugs).await;
    load_seed_file(&pool, Path::new("seeds/vyakarana.json")).await;

    let test_cases = vec![
        ("a", "a", "ā"),
        ("a", "i", "e"),
        ("a", "u", "o"),
        ("a", "e", "ai"),
        ("a", "o", "au"),
        ("i", "a", "ya"),
        ("u", "a", "va"),
        ("i", "i", "ī"),
        ("u", "u", "ū"),
        ("ṛ", "a", "ra"),
    ];

    let engine = vidya::engine::Engine::new();

    for (first, second, expected) in &test_cases {
        let request = vidya::engine::DeriveRequest {
            domain_id: vidya::db::get_domain_by_slug(&pool, slug)
                .await
                .unwrap()
                .unwrap()
                .id,
            domain_slug: slug.into(),
            operation: "sandhi".into(),
            input: json!({ "first": first, "second": second }),
        };

        let result = engine.derive(&pool, request).await.unwrap_or_else(|e| {
            panic!("{first} + {second}: {e}");
        });

        let actual = result.output["result"].as_str().unwrap();
        assert_eq!(
            actual, *expected,
            "{first} + {second} → {actual} (expected {expected})"
        );

        // Verify trace has rule name, sutra ref, input state, output state
        assert!(!result.trace.is_empty(), "{first} + {second}: no trace steps");
        for step in &result.trace {
            assert!(!step.rule.is_empty(), "trace step should have rule name");
            assert!(step.rule_ref.is_some(), "trace step should have sūtra reference");
            assert!(!step.input_state.is_empty(), "trace step should have input state");
            assert!(!step.output_state.is_empty(), "trace step should have output state");
        }
    }

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, source_slugs).await;
}

#[tokio::test]
#[serial]
async fn derive_unknown_operation_returns_error() {
    let pool = test_pool().await;
    let slug = "vyakarana";
    let source_slugs = &["ashtadhyayi", "shiva-sutras"];

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, source_slugs).await;
    load_seed_file(&pool, Path::new("seeds/vyakarana.json")).await;

    let engine = vidya::engine::Engine::new();
    let domain = vidya::db::get_domain_by_slug(&pool, slug)
        .await
        .unwrap()
        .unwrap();

    let request = vidya::engine::DeriveRequest {
        domain_id: domain.id,
        domain_slug: slug.into(),
        operation: "nonexistent".into(),
        input: json!({}),
    };

    let err = engine
        .derive(&pool, request)
        .await
        .expect_err("unknown operation should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("nonexistent") || msg.contains("no strategy"),
        "error should mention the unknown operation: {msg}"
    );

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, source_slugs).await;
}

#[tokio::test]
#[serial]
async fn derive_sandhi_via_mcp_tool() {
    let pool = test_pool().await;
    let slug = "vyakarana";
    let source_slugs = &["ashtadhyayi", "shiva-sutras"];

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, source_slugs).await;
    load_seed_file(&pool, Path::new("seeds/vyakarana.json")).await;

    let result = tools::derive::handle(
        &pool,
        tools::DeriveArgs {
            domain: slug.into(),
            operation: "sandhi".into(),
            input: json!({ "first": "a", "second": "i" }),
        },
    )
    .await
    .expect("derive via MCP tool");

    assert_eq!(result.result["result"], "e");
    assert_eq!(result.domain, "vyakarana");
    assert_eq!(result.operation, "sandhi");
    assert!(!result.trace.is_empty());
    assert!(result.trace[0].rule_ref.as_deref().unwrap().contains("6.1.87"));

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, source_slugs).await;
}

#[tokio::test]
#[serial]
async fn derive_conflict_resolution_apavada_beats_utsarga() {
    let pool = test_pool().await;
    let slug = "vyakarana";
    let source_slugs = &["ashtadhyayi", "shiva-sutras"];

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, source_slugs).await;
    load_seed_file(&pool, Path::new("seeds/vyakarana.json")).await;

    // Add a fictional apavāda rule that matches the same input as guṇa (a + i).
    // The apavāda should win over the utsarga guṇa rule.
    let domain = vidya::db::get_domain_by_slug(&pool, slug)
        .await
        .unwrap()
        .unwrap();
    let template = vidya::db::get_claim_template(&pool, domain.id, "sandhi_rule")
        .await
        .unwrap()
        .unwrap();

    vidya::db::insert_claim(
        &pool,
        domain.id,
        template.id,
        json!({
            "first": "a", "second": "i", "result": "X",
            "sutra": "99.99.99", "sutra_position": "99.99.099", "rule_type": "apavāda"
        }),
        "active",
        "a + i → X (fictional apavāda override)",
    )
    .await
    .expect("insert apavāda rule");

    let engine = vidya::engine::Engine::new();

    let request = vidya::engine::DeriveRequest {
        domain_id: domain.id,
        domain_slug: slug.into(),
        operation: "sandhi".into(),
        input: json!({ "first": "a", "second": "i" }),
    };

    let result = engine.derive(&pool, request).await.expect("derive should succeed");
    assert_eq!(
        result.output["result"].as_str().unwrap(),
        "X",
        "apavāda rule should win over utsarga"
    );
    assert!(result.trace[0].rule.contains("apavāda"));

    cleanup(&pool, slug).await;
    cleanup_sources(&pool, source_slugs).await;
}
