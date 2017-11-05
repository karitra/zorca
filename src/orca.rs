use std::collections::HashMap;

use std::sync::RwLock;

use resources::Endpoint;


pub const DEFAULT_WEB_SCHEME: &str = "http";
pub const DEFAULT_WEB_PORT: u16 = 8877;
pub const DEFAULT_STATUS_PORT: u16 = 9878;


pub enum WebHandler {
    State,
    Info,
    Metrics
}

#[derive(Debug, Deserialize)]
pub struct AppState {
    profile: String,
    state: String,
    workers: i64,
    state_version: i64,
    time_stamp: i64,
}

// mapping: app -> state
pub type CommitedState = HashMap<String, AppState>;
// mapping: uuid -> Orca struct
pub type OrcasPod = HashMap<String, Orca>;
pub type SyncedOrcasPod = RwLock<OrcasPod>;


#[derive(Debug, Deserialize)]
pub struct Info {
    uptime: i64,
    version: String,
    uuid: String, // unused as we know uuid in advance
}

pub struct Orca {
    // Note: duplicated from cluster
    hostname: String,
    endpoints: Vec<Endpoint>,

    state: CommitedState,
    info: Info,
}
