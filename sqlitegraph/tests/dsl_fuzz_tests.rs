use rand::Rng;
use sqlitegraph::{DslResult, parse_dsl};

#[path = "fuzz_common.rs"]
mod fuzz_common;

#[test]
fn fuzz_dsl_random_token_sequences() {
    let iterations = fuzz_common::fuzz_iterations();
    let mut rng = fuzz_common::labeled_rng("dsl-fuzz");
    for _ in 0..iterations {
        let expr = random_expression(&mut rng);
        match parse_dsl(&expr) {
            DslResult::Pattern(query) => {
                // Ensure we can inspect leg count without panics.
                assert!(query.legs.len() <= 32);
            }
            DslResult::Pipeline(pipeline) => {
                assert!(!pipeline.steps.is_empty());
            }
            DslResult::Subgraph(request) => {
                assert!(request.depth <= 9);
            }
            DslResult::Error(message) => assert!(
                !message.is_empty(),
                "errors must include context for expression: {expr}"
            ),
        }
    }
}

fn random_expression(rng: &mut impl Rng) -> String {
    const TOKENS: &[&str] = &[
        "pattern",
        "CALLS",
        "USES",
        "CONTAINS",
        "->",
        "*2",
        "*3",
        "filter",
        "type=Fn",
        "type=Module",
        "3-hop",
        "5-hop type=Fn",
    ];
    let count = rng.gen_range(1..=12);
    let mut parts: Vec<String> = Vec::with_capacity(count);
    for _ in 0..count {
        let token = TOKENS[rng.gen_range(0..TOKENS.len())];
        match token {
            "*2" | "*3" => {
                if let Some(last) = parts.last_mut() {
                    last.push_str(token);
                    continue;
                }
            }
            _ => {}
        }
        parts.push(token.to_string());
    }
    parts.join(" ")
}
