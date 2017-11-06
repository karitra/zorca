use std::collections::HashMap;

use std::sync::RwLock;

use resources::Endpoint;


pub const DEFAULT_WEB_SCHEME: &str = "http";
pub const DEFAULT_WEB_PORT: u16 = 8877;
#[allow(dead_code)] pub const DEFAULT_STATUS_PORT: u16 = 9878;


#[allow(dead_code)]
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
// mapping: hostname -> Orca struct
pub type OrcasPod = HashMap<String, OrcaRecord>;
pub type SyncedOrcasPod = RwLock<OrcasPod>;


#[derive(Debug, Deserialize)]
pub struct Info {
    pub uptime: i64,
    pub version: String,
    pub uuid: String,
}

pub struct Orca {
    pub endpoints: Vec<Endpoint>,
    pub state: CommitedState,
    pub info: Info,
}

pub struct OrcaRecord {
    pub orca: Orca,
    pub update_timestamp: u64,
}
