use serde_json::json;
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
