use sqlitegraph::dsl::{parse_dsl, DslResult};

#[test]
fn test_correct_parse_examples() {
    match parse_dsl("CALLS->USES") {
        DslResult::Pattern(pattern) => assert_eq!(pattern.legs.len(), 2),
        other => panic!("unexpected result {other:?}"),
    }
    match parse_dsl("3-hop type=Type") {
        DslResult::Subgraph(req) => assert_eq!(req.depth, 3),
        _ => panic!("expected subgraph"),
    }
}

#[test]
fn test_invalid_input_errors() {
    match parse_dsl("unknown phrase") {
        DslResult::Error(msg) => assert!(msg.contains("unsupported")),
        _ => panic!("expected error"),
    }
}

#[test]
fn test_deterministic_structure() {
    let a = parse_dsl("CALLS->USES");
    let b = parse_dsl("CALLS->USES");
    assert!(matches!(a, DslResult::Pattern(_)));
    assert_eq!(format!("{:?}", a), format!("{:?}", b));
}

#[test]
fn test_roundtrip_pipeline_parse() {
    match parse_dsl("pattern CALLS*3 filter type=Module") {
        DslResult::Pipeline(pipeline) => assert_eq!(pipeline.steps.len(), 2),
        _ => panic!("expected pipeline"),
    }
}
