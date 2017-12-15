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

#[derive(Debug, Serialize, Deserialize)]
pub struct AppState {
    profile: String,
    state: String,
    workers: i64,
    state_version: i64,
    about_state: Option<String>,
    state_description: Option<String>,
    time_stamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InAppState {
    profile: String,
    workers: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommittedState {
    // mapping: app -> state
    state: HashMap<String, AppState>,
    version: i64,
    timestamp: i64,
}


// mapping: hostname -> Orca struct
pub type OrcasPod = HashMap<String, OrcaRecord>;
pub type SyncedOrcasPod = RwLock<OrcasPod>;

// pub type OrcasIncomingStates = HashMap<String, InAppState>;

pub type Apps = HashMap<String, AppStat>;
pub type SyncedApps = RwLock<Apps>;

// TODO: support of int values (counters).
pub type Metrics = HashMap<String, f64>;


pub trait AppsTrait {
    fn update(&mut self, pod: &OrcasPod);
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Info {
    pub uptime: i64,
    pub version: String,
    pub uuid: String,
    // TODO: add api version when orca with updated info handle will be widly deployed
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Orca {
    pub endpoints: Vec<Endpoint>,
    pub committed_state: CommittedState,
    pub info: Info,
    pub metrics: Metrics,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrcaRecord {
    pub orca: Orca,
    pub update_timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrcaInState {
    pub incoming_state: HashMap<String, InAppState>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HostDistribution {
    profile: String,
    state: String,
    state_version: i64,
    workers: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppStat {
    pub total_workers: i64,
    pub hosts: HashMap<String, HostDistribution>,
}

impl AppStat {
    pub fn new() -> AppStat {
        AppStat { total_workers: 0, hosts: HashMap::new() }
    }
}

impl AppsTrait for Apps {
    fn update(&mut self, pod: &OrcasPod) {
        self.clear();

        for (hostname, orca) in pod {
            for (app, state) in &orca.orca.committed_state.state {
                let record = self.entry(app.clone()).or_insert(AppStat::new());

                record.total_workers += state.workers;
                record.hosts.insert(
                    hostname.clone(),
                    HostDistribution {
                        profile: state.profile.clone(),
                        state: state.state.clone(),
                        state_version: state.state_version,
                        workers: state.workers,
                    }
                );
            } // for (app, state)
        } // for (hostname, orca)
    }
}
