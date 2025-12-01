
use std::collections::VecDeque;

use crate::SqliteGraphError;

use ahash::AHashSet;

pub type MatchResult = crate::pattern::PatternMatch;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandLineConfig {
    pub backend: String,
    pub database: String,
    pub command: String,
    pub command_args: Vec<String>,
}

impl CommandLineConfig {
    pub fn from_args(args: &[&str]) -> Result<Self, String> {
        let mut backend = String::from("sqlite");
        let mut database = String::from("memory");
        let mut command = String::from("status");
        let mut command_args = Vec::new();
        let mut command_set = false;
        let mut iter = args.iter().skip(1);
        while let Some(arg) = iter.next() {
            if command_set {
                command_args.push(arg.to_string());
                continue;
            }
            match *arg {
                "--backend" => {
                    backend = iter
                        .next()
                        .ok_or_else(|| "--backend requires a value".to_string())?
                        .to_string();
                }
                "--db" | "--database" => {
                    database = iter
                        .next()
                        .ok_or_else(|| "--db requires a value".to_string())?
                        .to_string();
                }
                "--command" => {
                    command = iter
                        .next()
                        .ok_or_else(|| "--command requires a value".to_string())?
                        .to_string();
                    command_set = true;
                }
                other if other.starts_with('-') => {
                    return Err(format!("unknown flag {other}"));
                }
                _ => {
                    command = arg.to_string();
                    command_set = true;
                }
            }
        }
        Ok(Self {
            backend,
            database,
            command,
            command_args,
        })
    }

    pub fn help() -> &'static str {
        "Usage: sqlitegraph [--backend sqlite] [--db memory|PATH] [--command status]
"
    }
}

#[derive(Clone, Debug)]
pub struct Constraint {
    pub allowed_edge_types: Vec<String>,
    pub allowed_node_types: Vec<String>,
}

pub fn into_lookup(values: &[String]) -> Option<AHashSet<&str>> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().map(|s| s.as_str()).collect())
    }
}
