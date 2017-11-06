#[derive(Debug, Deserialize, Clone)]
pub struct Endpoint(pub String, pub u16);

impl Endpoint {
    pub fn host_str(&self) -> String { self.0.clone() }
}

#[derive(Debug, Deserialize)]
pub struct Resources {
    pub cpu: i64,
    pub mem: i64,
}

#[derive(Debug, Deserialize)]
pub struct NodeInfo {
    pub hostname: String,
    pub resources: Resources,
    pub endpoints: Vec<Endpoint>
}
