use crate::pattern::PatternMatch;

pub type MatchResult = PatternMatch;

#[derive(Clone, Debug)]
pub struct Constraint {
    pub allowed_edge_types: Vec<String>,
    pub allowed_node_types: Vec<String>,
}
