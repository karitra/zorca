use std::collections::{
    HashMap,
    HashSet
};

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
    time_stamp: i64,
}

// mapping: app -> state
pub type CommitedState = HashMap<String, AppState>;

// mapping: hostname -> Orca struct
pub type OrcasPod = HashMap<String, OrcaRecord>;
pub type SyncedOrcasPod = RwLock<OrcasPod>;

pub type Apps = HashMap<String, AppStat>;
pub type SyncedApps = RwLock<Apps>;

pub trait AppsTrait {
    fn update(&mut self, pod: &OrcasPod);
}


#[derive(Debug, Serialize, Deserialize)]
pub struct Info {
    pub uptime: i64,
    pub version: String,
    pub uuid: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Orca {
    pub endpoints: Vec<Endpoint>,
    pub state: CommitedState,
    pub info: Info,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrcaRecord {
    pub orca: Orca,
    pub update_timestamp: u64,
}

type HostsAndWorkers = (String, i64);

#[derive(Debug, Serialize, Deserialize)]
pub struct AppStat {
    pub total_workers: i64,
    pub profiles: HashSet<String>,
    pub hosts: HashSet<HostsAndWorkers>,
}

impl AppStat {
    pub fn new() -> AppStat {
        AppStat { total_workers: 0, profiles: HashSet::new(), hosts: HashSet::new() }
    }
}

impl AppsTrait for Apps {
    fn update(&mut self, pod: &OrcasPod) {
        self.clear();

        for (hostname, orca) in pod {
            for (app, state) in &orca.orca.state {
                let record = self.entry(app.clone()).or_insert(AppStat::new());

                record.total_workers += state.workers;
                record.profiles.insert(state.profile.clone());

                record.hosts.insert((hostname.clone(), state.workers));
            }
        }
    }
}
