use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Endpoint(String, u16);

#[derive(Debug, Deserialize)]
struct Resources {
    cpu: i64,
    mem: i64,
}

#[derive(Debug, Deserialize)]
pub struct NodeInfo {
    hostname: String,
    resources: Resources,
    endpoints: Vec<Endpoint>
}
