use std::collections::HashMap;

use std::sync::RwLock;

use resources::Endpoint;


pub const DEFAULT_WEB_SCHEME: &str = "http";
pub const DEFAULT_WEB_PORT: u16 = 8877;
#[allow(dead_code)] pub const DEFAULT_STATUS_PORT: u16 = 9878;

pub const STARTED_STATE: &str = "STARTED";


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
pub struct CommittedState {
    // mapping: app -> state
    state: HashMap<String, AppState>,
    version: i64,
    timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InAppState {
    profile: String,
    workers: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IncomingState {
    state: HashMap<String, InAppState>,
    timestamp: i64,
    version: i64,
}

impl IncomingState {
    pub fn new() -> IncomingState {
        IncomingState {
            state: HashMap::new(),
            timestamp: 0,
            version: -1,
        }
    }
}

pub type WorkersDistribution = HashMap<String, i64>;

// mapping: hostname -> Orca struct
pub type OrcasPod = HashMap<String, OrcaRecord>;
pub type SyncedOrcasPod = RwLock<OrcasPod>;

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkersCount {
    pub input: i64,
    pub output: i64,
    pub runtime: i64,
    pub mismatch_inout: bool,
    pub mismatch_runtime: bool,
}

impl WorkersCount {
    fn new() -> WorkersCount {
        WorkersCount{
            input: 0,
            output: 0,
            runtime: 0,
            mismatch_inout: false,
            mismatch_runtime: false,
        }
    }

    fn nonempty(&self) -> bool {
        ! (self.input == 0 && self.output == 0 && self.runtime == 0)
    }
}

pub type Distribution = HashMap<String, WorkersCount>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Orca {
    pub endpoints: Vec<Endpoint>,
    pub info: Info,
    pub metrics: Metrics,
    pub mismatched: Distribution,
    #[serde(skip_serializing)]
    pub distribution: Distribution,
    #[serde(skip_serializing)]
    pub committed_state: CommittedState,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrcaRecord {
    pub orca: Orca,
    pub update_timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrcaInState {
    pub state: HashMap<String, InAppState>,
    pub timestamp: u64,
    pub version: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppStat {
    pub total_workers: i64,
    pub hosts: HashMap<String, WorkersCount>,
}

impl AppStat {
    pub fn new() -> AppStat {
        AppStat { total_workers: 0, hosts: HashMap::new() }
    }
}

impl AppsTrait for Apps {
    fn update(&mut self, pod: &OrcasPod) {
        self.clear();

        for (host, orca) in pod {
            for (app, dist) in orca.orca.distribution.iter()
                .filter(|&(_, dist)| dist.nonempty()) {
                    let record = self.entry(app.clone()).or_insert(AppStat::new());
                    record.hosts.insert(host.clone(), dist.clone());
            }
        }
    }
}

pub fn make_workers_distribution(
    in_state: &IncomingState,
    out_state: &CommittedState,
    real_state: &WorkersDistribution)
    -> Distribution
{
    let mut distribution : Distribution = HashMap::new();

    for (app, record) in out_state.state
        .iter().filter(|&(_,v)| v.state == STARTED_STATE)
    {
        let r = distribution.entry(app.to_owned()).or_insert(WorkersCount::new());
        r.output = record.workers;
    }

    for (app, record) in &out_state.state {
        if record.state == STARTED_STATE {
            let r = distribution.entry(app.to_owned()).or_insert(WorkersCount::new());
            r.output = record.workers;
        }
    }

    for (app, record) in &in_state.state {
        let r = distribution.entry(app.to_owned()).or_insert(WorkersCount::new());
        r.input = record.workers;
    }

    for (app, count) in real_state {
        let r = distribution.entry(app.to_owned()).or_insert(WorkersCount::new());
        r.runtime = *count;
    }

    for (_, rec) in distribution.iter_mut() {
        if rec.input != rec.output {
            rec.mismatch_inout = true;
        }

        if rec.input != rec.runtime {
            rec.mismatch_runtime = true;
        }
    }

    distribution
}

pub fn make_mismatched_list(d: &Distribution) -> Distribution {
    d.iter()
        .filter(|&(_,v)| v.mismatch_runtime || v.mismatch_inout)
        .map(|(k,v)| (k.clone(), v.clone()))
        .collect()
}
