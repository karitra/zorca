use std::net::IpAddr;
use std::collections::HashMap;

use cocaine::service::Unicorn;
use serde_derive::Deserialize;

#[dereve(Debug, Deserialize)]
struct Endpoint(String, u16);

#[dereve(Debug, Deserialize)]
struct Resource {
    cpu: i64,
    mem: i64,
    net: i64,
    endpoints: Vec<Endpoint>,
}

struct Node {
    endpoints: Vec<IpAddr>
}

type Nodes = HashMap<String,Node>;


fn make_state(unicorn: &Unicorn, nodes: Vec<String>)
    -> Nodes
{
    let mut futures = Vec::new();
    for node in &nodes {

        let completion = unicorn.get::<Resource>(node)
            .and_then(move |version, resource| {
                println!(" get node rcord {:?}", resource);
                for ep in &resource.endpoints {

                }
                Ok(())
        });

        futures.push(completion);
    }

    let result = futures::future::join_all(futures);
    HashMap::new()
}
