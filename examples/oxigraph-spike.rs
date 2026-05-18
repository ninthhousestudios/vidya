use oxigraph::io::RdfFormat;
use oxigraph::model::Term;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // In-memory store for the spike (persistent would be Store::open("path"))
    let store = Store::new()?;

    // Load the Turtle file
    let ttl = fs::read_to_string("examples/jyotish-spike.ttl")?;
    store.load_from_reader(RdfFormat::Turtle, ttl.as_bytes())?;

    let count = store.len()?;
    println!("Loaded {count} triples\n");

    // ── Query 1: List all grahas with their nature ──
    println!("=== All Grahas ===");
    let q1 = r#"
        PREFIX jyotish: <http://vidya.ninthhouse.studio/domain/jyotish/>
        PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

        SELECT ?label ?nature ?element
        WHERE {
            ?graha a jyotish:Graha ;
                   rdfs:label ?label ;
                   jyotish:nature ?nature ;
                   jyotish:element ?element .
        }
        ORDER BY ?label
    "#;
    print_query(&store, q1)?;

    // ── Query 2: Where is Sūrya exalted? ──
    println!("\n=== Sūrya's exaltation ===");
    let q2 = r#"
        PREFIX jyotish: <http://vidya.ninthhouse.studio/domain/jyotish/>
        PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

        SELECT ?rashi_label
        WHERE {
            jyotish:surya jyotish:exaltedIn ?rashi .
            ?rashi rdfs:label ?rashi_label .
        }
    "#;
    print_query(&store, q2)?;

    // ── Query 3: Natural friends of Guru ──
    println!("\n=== Guru's natural friends ===");
    let q3 = r#"
        PREFIX jyotish: <http://vidya.ninthhouse.studio/domain/jyotish/>
        PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

        SELECT ?friend_label
        WHERE {
            jyotish:guru jyotish:naturalFriend ?friend .
            ?friend rdfs:label ?friend_label .
        }
        ORDER BY ?friend_label
    "#;
    print_query(&store, q3)?;

    // ── Query 4: 2-hop friend traversal from Sūrya ──
    println!("\n=== Friends-of-friends of Sūrya (2 hops) ===");
    let q4 = r#"
        PREFIX jyotish: <http://vidya.ninthhouse.studio/domain/jyotish/>
        PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

        SELECT DISTINCT ?name ?depth
        WHERE {
            {
                jyotish:surya jyotish:naturalFriend ?f .
                ?f rdfs:label ?name .
                BIND(1 AS ?depth)
            }
            UNION
            {
                jyotish:surya jyotish:naturalFriend/jyotish:naturalFriend ?f .
                FILTER(?f != jyotish:surya)
                ?f rdfs:label ?name .
                BIND(2 AS ?depth)
            }
        }
        ORDER BY ?depth ?name
    "#;
    print_query(&store, q4)?;

    // ── Query 5: All fire-element entities (cross-type) ──
    println!("\n=== All fire-element entities ===");
    let q5 = r#"
        PREFIX jyotish: <http://vidya.ninthhouse.studio/domain/jyotish/>
        PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

        SELECT ?label ?type
        WHERE {
            ?entity jyotish:element "fire" ;
                    rdfs:label ?label ;
                    a ?type .
        }
        ORDER BY ?type ?label
    "#;
    print_query(&store, q5)?;

    // ── Query 6: RDF-star provenance — who asserts Sūrya's exaltation? ──
    println!("\n=== Provenance: Sūrya exalted in Meṣa ===");
    let q6 = r#"
        PREFIX jyotish: <http://vidya.ninthhouse.studio/domain/jyotish/>
        PREFIX vidya: <http://vidya.ninthhouse.studio/ontology/>
        PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

        SELECT ?degree ?tradition_label ?source_label ?pramana ?confidence
        WHERE {
            << jyotish:surya jyotish:exaltedIn jyotish:mesha >>
                vidya:exaltationDegree ?degree ;
                vidya:assertedBy ?assertion .
            ?assertion vidya:tradition ?trad ;
                       vidya:source ?src ;
                       vidya:pramana ?pramana ;
                       vidya:confidence ?confidence .
            ?trad rdfs:label ?tradition_label .
            ?src  rdfs:label ?source_label .
        }
    "#;
    print_query(&store, q6)?;

    // ── Query 7: All exaltations with provenance (cross-cutting) ──
    println!("\n=== All exaltations with provenance ===");
    let q7 = r#"
        PREFIX jyotish: <http://vidya.ninthhouse.studio/domain/jyotish/>
        PREFIX vidya: <http://vidya.ninthhouse.studio/ontology/>
        PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

        SELECT ?graha_label ?rashi_label ?degree ?pramana ?confidence
        WHERE {
            ?graha jyotish:exaltedIn ?rashi .
            ?graha rdfs:label ?graha_label .
            ?rashi rdfs:label ?rashi_label .
            OPTIONAL {
                << ?graha jyotish:exaltedIn ?rashi >>
                    vidya:exaltationDegree ?degree ;
                    vidya:assertedBy ?a .
                ?a vidya:pramana ?pramana ;
                   vidya:confidence ?confidence .
            }
        }
        ORDER BY ?graha_label
    "#;
    print_query(&store, q7)?;

    println!("\n✓ Spike complete — all queries executed successfully");
    Ok(())
}

fn print_query(store: &Store, query: &str) -> Result<(), Box<dyn std::error::Error>> {
    match SparqlEvaluator::new().parse_query(query)?.on_store(store).execute()? {
        QueryResults::Solutions(solutions) => {
            let vars: Vec<String> = solutions
                .variables()
                .iter()
                .map(|v| v.as_str().to_string())
                .collect();
            println!("  {}", vars.join(" | "));
            println!("  {}", vars.iter().map(|v| "-".repeat(v.len().max(12))).collect::<Vec<_>>().join("-+-"));

            for solution in solutions {
                let solution = solution?;
                let row: Vec<String> = vars
                    .iter()
                    .map(|v| {
                        solution
                            .get(v.as_str())
                            .map(|term| match term {
                                Term::Literal(lit) => lit.value().to_string(),
                                Term::NamedNode(nn) => {
                                    let iri = nn.as_str();
                                    iri.rsplit('/').next().unwrap_or(iri).to_string()
                                }
                                Term::BlankNode(bn) => format!("_:{}", bn.as_str()),
                                Term::Triple(t) => format!("<< {} >>", t),
                            })
                            .unwrap_or_else(|| "—".to_string())
                    })
                    .collect();
                println!("  {}", row.join(" | "));
            }
        }
        _ => println!("  (non-SELECT result)"),
    }
    Ok(())
}
