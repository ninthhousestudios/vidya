use anyhow::{Context, Result};

use vidya::{
    config::{Config, vidya_home},
    db,
    engine,
};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::from_path(vidya_home().join(".env"));
    let _ = dotenvy::dotenv();

    let cfg = Config::from_env();
    let pool = db::connect(&cfg).await.context("connecting to database")?;

    let domain = db::get_domain_by_slug(&pool, "vyakarana")
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .ok_or_else(|| anyhow::anyhow!("vyakarana domain not found"))?;

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

    println!("Sandhi derivation tests:");
    println!("{:-<60}", "");

    let mut passed = 0;
    let mut failed = 0;

    for (first, second, expected) in test_cases {
        let request = engine::DeriveRequest {
            domain_id: domain.id,
            domain_slug: "vyakarana".into(),
            operation: "sandhi".into(),
            input: serde_json::json!({ "first": first, "second": second }),
        };

        let result = engine::derive(&pool, request)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let actual = result.output["result"].as_str().unwrap_or("???");
        let ok = actual == expected;

        if ok {
            passed += 1;
            println!("  PASS  {} + {} → {} (expected {})", first, second, actual, expected);
        } else {
            failed += 1;
            println!("  FAIL  {} + {} → {} (expected {})", first, second, actual, expected);
        }

        for step in &result.trace {
            println!("        step {}: {} → {}  [{}]",
                step.step,
                step.input_state,
                step.output_state,
                step.rule_ref.as_deref().unwrap_or("?"),
            );
        }
    }

    println!("{:-<60}", "");
    println!("{passed} passed, {failed} failed");

    Ok(())
}
