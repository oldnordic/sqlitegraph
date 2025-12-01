use sqlitegraph::{
    dsl::{DslResult, parse_dsl},
    pattern::PatternQuery,
    pipeline::{ReasoningPipeline, ReasoningStep},
};

#[test]
fn parses_arrow_chain_into_pattern_query() {
    match parse_dsl("CALLS->USES") {
        DslResult::Pattern(pattern) => {
            assert_eq!(pattern.legs.len(), 2);
            assert_eq!(pattern.legs[0].edge_type.as_deref(), Some("CALLS"));
            assert_eq!(pattern.legs[1].edge_type.as_deref(), Some("USES"));
        }
        other => panic!("expected pattern, got {:?}", other),
    }
}

#[test]
fn parses_repetition_without_arrow() {
    match parse_dsl("CALLS*3") {
        DslResult::Pattern(pattern) => assert_eq!(pattern.legs.len(), 3),
        other => panic!("expected pattern, got {:?}", other),
    }
}

#[test]
fn parses_hop_command_into_subgraph() {
    match parse_dsl("3-hop type=Fn") {
        DslResult::Subgraph(request) => {
            assert_eq!(request.depth, 3);
            assert_eq!(request.allowed_node_types, vec!["Fn".to_string()]);
        }
        other => panic!("expected subgraph, got {:?}", other),
    }
}

#[test]
fn parses_pattern_pipeline_with_filter() {
    match parse_dsl("pattern CALLS*2 filter type=Module") {
        DslResult::Pipeline(pipeline) => {
            assert_eq!(pipeline.steps.len(), 2);
            assert_pattern(&pipeline, 2);
            match &pipeline.steps[1] {
                ReasoningStep::Filter(constraint) => {
                    assert_eq!(constraint.kind.as_deref(), Some("Module"));
                }
                other => panic!("expected filter step, got {:?}", other),
            }
        }
        other => panic!("expected pipeline, got {:?}", other),
    }
}

#[test]
fn rejects_empty_input() {
    match parse_dsl("   ") {
        DslResult::Error(msg) => assert!(msg.contains("empty")),
        other => panic!("expected error, got {:?}", other),
    }
}

#[test]
fn rejects_unknown_token() {
    match parse_dsl("UNKNOWN") {
        DslResult::Error(msg) => assert!(msg.contains("unsupported")),
        other => panic!("expected error, got {:?}", other),
    }
}

#[test]
fn rejects_conflicting_filter_clauses() {
    match parse_dsl("pattern CALLS filter type=Fn filter type=Module") {
        DslResult::Error(msg) => assert!(msg.contains("filter")),
        other => panic!("expected error, got {:?}", other),
    }
}

fn assert_pattern(pipeline: &ReasoningPipeline, expected_legs: usize) {
    match &pipeline.steps[0] {
        ReasoningStep::Pattern(PatternQuery { legs, .. }) => assert_eq!(legs.len(), expected_legs),
        other => panic!("expected pattern step, got {:?}", other),
    }
}
