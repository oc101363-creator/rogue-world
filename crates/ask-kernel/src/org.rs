//! Organization graph meta — MVP-0 stub only.

#[derive(Clone, Debug, Default)]
pub struct OrgGraph {
    pub nodes: Vec<String>,
    pub edges: Vec<(usize, usize)>,
}

pub trait OrgPort {
    fn graph(&self) -> &OrgGraph;
}

pub struct NullOrg {
    graph: OrgGraph,
}

impl Default for NullOrg {
    fn default() -> Self {
        Self {
            graph: OrgGraph::default(),
        }
    }
}

impl OrgPort for NullOrg {
    fn graph(&self) -> &OrgGraph {
        &self.graph
    }
}
