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
