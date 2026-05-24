use vidya_core::vsa::{BinaryBipolar, EntityIndex, Hrr, VsaOps};
use vidya_core::KnowledgeStore;

const DOMAIN: &str = "jyotish";
const JYOTISH_TTL: &str = include_str!("../../seeds/jyotish.ttl");

const DOMAIN_BASE: &str = "http://vidya.ninthhouse.studio/domain/jyotish/";

fn iri(local: &str) -> String {
    format!("{DOMAIN_BASE}{local}")
}

fn short(full_iri: &str) -> &str {
    full_iri
        .strip_prefix(DOMAIN_BASE)
        .or_else(|| full_iri.rsplit('/').next())
        .unwrap_or(full_iri)
}

fn build_store() -> KnowledgeStore {
    let store = KnowledgeStore::new_memory().unwrap();
    store.load_domain(DOMAIN, JYOTISH_TTL).unwrap();
    store
}

fn build_index<A: VsaOps>(ops: A, store: &KnowledgeStore) -> EntityIndex<A> {
    EntityIndex::build(ops, store.inner(), DOMAIN)
}

const GRAHAS: &[&str] = &[
    "surya", "chandra", "mangala", "budha", "guru", "shukra", "shani", "rahu", "ketu",
];

// =========================================================================
// Experiment 1: Graha similarity matrix
// =========================================================================

fn exp1_similarity_matrix<A: VsaOps>(label: &str, index: &EntityIndex<A>) {
    println!("\n=== Experiment 1: Graha similarity matrix ({label}) ===\n");

    // Header
    print!("{:>12}", "");
    for g in GRAHAS {
        print!("{:>9}", g);
    }
    println!();

    for &ga in GRAHAS {
        print!("{:>12}", ga);
        for &gb in GRAHAS {
            let sim = index
                .entity_similarity(&iri(ga), &iri(gb))
                .unwrap_or(f64::NAN);
            print!("{:>9.3}", sim);
        }
        println!();
    }

    // Key pairs to check
    println!("\n  Key similarity checks:");
    let checks = [
        ("surya", "mangala", "both male, fire, malefic"),
        ("mangala", "shukra", "opposite gender/element/nature"),
        ("surya", "chandra", "both luminaries but different"),
        ("rahu", "ketu", "both shadow planets"),
        ("guru", "shukra", "both benefic but different"),
    ];
    for (a, b, reason) in &checks {
        let sim = index
            .entity_similarity(&iri(a), &iri(b))
            .unwrap_or(f64::NAN);
        println!("    {a:>8} ~ {b:<8} = {sim:.4}  ({reason})");
    }
}

// =========================================================================
// Experiment 2: Sign clustering (Mars-ruled signs)
// =========================================================================

fn exp2_sign_clustering<A: VsaOps>(label: &str, index: &EntityIndex<A>) {
    println!("\n=== Experiment 2: Sign clustering ({label}) ===\n");

    let mars_ruled = [("mesha", "Aries"), ("vrischika", "Scorpio")];
    let venus_ruled = [("vrishabha", "Taurus"), ("tula", "Libra")];

    let sim_mars = index
        .entity_similarity(&iri(mars_ruled[0].0), &iri(mars_ruled[1].0))
        .unwrap_or(f64::NAN);
    let sim_venus = index
        .entity_similarity(&iri(venus_ruled[0].0), &iri(venus_ruled[1].0))
        .unwrap_or(f64::NAN);
    let sim_cross = index
        .entity_similarity(&iri(mars_ruled[0].0), &iri(venus_ruled[0].0))
        .unwrap_or(f64::NAN);

    println!("  Mars-ruled signs (mesha ~ vrischika):   {sim_mars:.4}");
    println!("  Venus-ruled signs (vrishabha ~ tula):    {sim_venus:.4}");
    println!("  Cross-ruler (mesha ~ vrishabha):          {sim_cross:.4}");
    println!();

    if sim_mars > sim_cross {
        println!("  PASS: same-ruler signs are more similar than cross-ruler");
    } else {
        println!("  FAIL: same-ruler signs are NOT more similar than cross-ruler");
    }
}

// =========================================================================
// Experiment 3: Role-filler recovery (unbind)
// =========================================================================

fn exp3_role_filler_recovery<A: VsaOps>(label: &str, index: &EntityIndex<A>) {
    println!("\n=== Experiment 3: Role-filler recovery ({label}) ===\n");

    // mangala rules mesha and vrischika
    let rules_iri = iri("rules");
    let mangala_iri = iri("mangala");

    println!("  unbind(mangala, rules) — expecting mesha, vrischika in top results:");
    let results = index.unbind_query(&mangala_iri, &rules_iri, 10);
    for (i, (result_iri, sim)) in results.iter().enumerate() {
        let name = short(result_iri);
        let marker = if name == "mesha" || name == "vrischika" {
            " <<<"
        } else {
            ""
        };
        println!("    #{}: {:<30} sim={:.4}{}", i + 1, name, sim, marker);
    }

    let target_names: Vec<&str> = results.iter().map(|(iri, _)| short(iri)).collect();
    let mesha_found = target_names.iter().position(|&n| n == "mesha");
    let vrischika_found = target_names.iter().position(|&n| n == "vrischika");

    println!();
    match (mesha_found, vrischika_found) {
        (Some(m), Some(v)) => {
            println!("  mesha at #{}, vrischika at #{}", m + 1, v + 1);
            if m < 5 || v < 5 {
                println!("  PASS: at least one target in top 5");
            } else {
                println!("  MARGINAL: both targets found but outside top 5");
            }
        }
        (Some(m), None) => println!("  PARTIAL: mesha at #{}, vrischika not in top 10", m + 1),
        (None, Some(v)) => println!("  PARTIAL: vrischika at #{}, mesha not in top 10", v + 1),
        (None, None) => println!("  FAIL: neither mesha nor vrischika in top 10"),
    }

    // Also test: surya exaltedIn — should return mesha
    println!();
    let exalted_iri = iri("exaltedIn");
    let surya_iri = iri("surya");
    println!("  unbind(surya, exaltedIn) — expecting mesha:");
    let results = index.unbind_query(&surya_iri, &exalted_iri, 5);
    for (i, (result_iri, sim)) in results.iter().enumerate() {
        let name = short(result_iri);
        let marker = if name == "mesha" { " <<<" } else { "" };
        println!("    #{}: {:<30} sim={:.4}{}", i + 1, name, sim, marker);
    }
}

// =========================================================================
// Experiment 4: Dimensionality sweep
// =========================================================================

fn exp4_dimensionality_sweep() {
    println!("\n=== Experiment 4: Dimensionality sweep ===\n");

    let store = build_store();

    println!("  --- Binary Bipolar ---");
    println!("  {:>8} {:>10} {:>10} {:>12} {:>12} {:>14}",
        "bits", "entities", "symbols", "sun~mars", "mars~venus", "mesha~vrisch");

    for dim in [512, 1024, 2048, 4096, 8192] {
        let ops = BinaryBipolar::new(dim);
        let index = build_index(ops, &store);
        let sun_mars = index.entity_similarity(&iri("surya"), &iri("mangala")).unwrap_or(f64::NAN);
        let mars_venus = index.entity_similarity(&iri("mangala"), &iri("shukra")).unwrap_or(f64::NAN);
        let mesha_vr = index.entity_similarity(&iri("mesha"), &iri("vrischika")).unwrap_or(f64::NAN);
        println!("  {:>8} {:>10} {:>10} {:>12.4} {:>12.4} {:>14.4}",
            dim, index.entity_count(), index.symbol_count(), sun_mars, mars_venus, mesha_vr);
    }

    println!("\n  --- HRR ---");
    println!("  {:>8} {:>10} {:>10} {:>12} {:>12} {:>14}",
        "dims", "entities", "symbols", "sun~mars", "mars~venus", "mesha~vrisch");

    for dim in [128, 256, 512, 1024, 2048] {
        let ops = Hrr::new(dim);
        let index = build_index(ops, &store);
        let sun_mars = index.entity_similarity(&iri("surya"), &iri("mangala")).unwrap_or(f64::NAN);
        let mars_venus = index.entity_similarity(&iri("mangala"), &iri("shukra")).unwrap_or(f64::NAN);
        let mesha_vr = index.entity_similarity(&iri("mesha"), &iri("vrischika")).unwrap_or(f64::NAN);
        println!("  {:>8} {:>10} {:>10} {:>12.4} {:>12.4} {:>14.4}",
            dim, index.entity_count(), index.symbol_count(), sun_mars, mars_venus, mesha_vr);
    }
}

// =========================================================================
// Experiment 5: Top-k similar entities for key entities
// =========================================================================

fn exp5_top_similar<A: VsaOps>(label: &str, index: &EntityIndex<A>) {
    println!("\n=== Experiment 5: Top similar entities ({label}) ===\n");

    let queries = ["mangala", "mesha", "guru", "rahu"];
    for &entity in &queries {
        println!("  Top 5 similar to {entity}:");
        let results = index.similar(&iri(entity), 5);
        for (i, (result_iri, sim)) in results.iter().enumerate() {
            println!("    #{}: {:<25} sim={:.4}", i + 1, short(result_iri), sim);
        }
        println!();
    }
}

// =========================================================================
// Main test
// =========================================================================

#[test]
fn vsa_spike_full_validation() {
    let store = build_store();

    println!("\n{}", "=".repeat(72));
    println!("VSA SPIKE: Entity Similarity Validation");
    println!("Domain: jyotish (Vedic astrology)");
    println!("{}", "=".repeat(72));

    // Binary bipolar at 4096 bits
    let binary_ops = BinaryBipolar::new(4096);
    let binary_index = build_index(binary_ops, &store);
    println!("\nBinary index: {} entities, {} symbols", binary_index.entity_count(), binary_index.symbol_count());

    // HRR at 1024 dimensions
    let hrr_ops = Hrr::new(1024);
    let hrr_index = build_index(hrr_ops, &store);
    println!("HRR index: {} entities, {} symbols", hrr_index.entity_count(), hrr_index.symbol_count());

    // Run experiments with both algebras
    exp1_similarity_matrix("Binary 4096-bit", &binary_index);
    exp1_similarity_matrix("HRR 1024-dim", &hrr_index);

    exp2_sign_clustering("Binary 4096-bit", &binary_index);
    exp2_sign_clustering("HRR 1024-dim", &hrr_index);

    exp3_role_filler_recovery("Binary 4096-bit", &binary_index);
    exp3_role_filler_recovery("HRR 1024-dim", &hrr_index);

    exp4_dimensionality_sweep();

    exp5_top_similar("Binary 4096-bit", &binary_index);
    exp5_top_similar("HRR 1024-dim", &hrr_index);

    println!("\n{}", "=".repeat(72));
    println!("END OF VSA SPIKE VALIDATION");
    println!("{}", "=".repeat(72));
}
