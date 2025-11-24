use crate::{
    SqliteGraphError,
    backend::{EdgeSpec, GraphBackend, NeighborQuery, NodeSpec},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandLineConfig {
    pub backend: String,
    pub database: String,
    pub command: String,
}

impl CommandLineConfig {
    pub fn from_args(args: &[&str]) -> Result<Self, String> {
        let mut backend = String::from("sqlite");
        let mut database = String::from("memory");
        let mut command = String::from("status");
        let mut iter = args.iter().skip(1);
        while let Some(arg) = iter.next() {
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
                }
                other if other.starts_with('-') => {
                    return Err(format!("unknown flag {other}"));
                }
                _ => {
                    command = arg.to_string();
                }
            }
        }
        Ok(Self {
            backend,
            database,
            command,
        })
    }

    pub fn help() -> &'static str {
        "Usage: sqlitegraph [--backend sqlite] [--db memory|PATH] [--command status]\n"
    }
}

pub struct BackendClient<B> {
    backend: B,
}

impl<B> BackendClient<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }
}

impl<B> BackendClient<B>
where
    B: GraphBackend,
{
    pub fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        self.backend.insert_node(node)
    }

    pub fn insert_edge(&self, edge: EdgeSpec) -> Result<i64, SqliteGraphError> {
        self.backend.insert_edge(edge)
    }

    pub fn neighbors(&self, node: i64, query: NeighborQuery) -> Result<Vec<i64>, SqliteGraphError> {
        self.backend.neighbors(node, query)
    }

    pub fn bfs(&self, start: i64, depth: u32) -> Result<Vec<i64>, SqliteGraphError> {
        self.backend.bfs(start, depth)
    }

    pub fn shortest_path(
        &self,
        start: i64,
        end: i64,
    ) -> Result<Option<Vec<i64>>, SqliteGraphError> {
        self.backend.shortest_path(start, end)
    }
}
