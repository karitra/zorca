#[derive(Debug, Deserialize, Clone)]
pub struct Endpoint(String, u16);

#[derive(Debug, Deserialize)]
pub struct Resources {
    cpu: i64,
    mem: i64,
}

#[derive(Debug, Deserialize)]
pub struct NodeInfo {
    pub hostname: String,
    pub resources: Resources,
    pub endpoints: Vec<Endpoint>
}

impl Endpoint {
    pub fn host_str(&self) -> String { self.0.clone() }
}
