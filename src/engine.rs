use cocaine::Service;
use cocaine::hpack::RawHeader;

use tokio_core::reactor::{Core, Handle, Timeout};

use hyper;
// use hyper::client::Connect;

use futures::future;
use futures::{Future, Stream};
use futures::sync::mpsc;

use serde;
use serde_json;

use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, RwLock};
use std::str::FromStr;
use std::time;
use std::iter::Iterator;
use std::net;
use std::collections::HashMap;

use secure::make_ticket_service;
use errors::CombinedError;
use types::SubscribeMessage;
use config::Config;
use resources::{Endpoint, NodeInfo};

use unicorn::{
    kids_subscribe,
    get_node
};


use orca;
use orca::{
    SyncedOrcasPod,
    DEFAULT_WEB_PORT
};


const GATHER_INTERVAL_SEC: u64 = 1;


pub type Cluster = HashMap<String, NodeInfo>;
pub type SyncedCluster = RwLock<Cluster>;
type AuthHeaders = Vec<RawHeader>;
type UuidNodeInfo = (String, Option<NodeInfo>);


trait ClusterInterface {
    fn update(&mut self, nodes: Vec<UuidNodeInfo>);
    fn endpoints(&self) -> HashMap<String, Vec<Endpoint>>;
}

impl ClusterInterface for Cluster {
    fn update(&mut self, nodes: Vec<UuidNodeInfo>) {
        self.clear();
        self.reserve(nodes.len());

        for node in nodes {
            let (uuid, info) = node;
            if let Some(info) = info {
                self.insert(uuid, info);
            }
        } // for
    }

    fn endpoints(&self) -> HashMap<String, Vec<Endpoint>> {
        let mut endpoints: HashMap<String, Vec<Endpoint>> = HashMap::with_capacity(self.len());
        for (uuid, node_info) in self {
            endpoints.entry(uuid.clone()).or_insert(node_info.endpoints.clone());
        }
        endpoints
    }
}


fn make_auth_headers(header: Option<String>) -> Option<AuthHeaders>  {
    header.and_then(|hdr|
        Some(vec![ RawHeader::new("authorization".as_bytes(), hdr.into_bytes()) ])
    )
}


pub fn subscription<'a>(config: &Config, path: &'a str, cluster: Arc<SyncedCluster>) {
    let mut core = Core::new().unwrap();
    let proxy = make_ticket_service(Service::new("tvm", &core.handle()), &config);
    let proxy = Rc::new(RefCell::new(proxy));

    let (tx, rx) = mpsc::unbounded::<SubscribeMessage>();

    let handle = core.handle();

    let subscibe_future = proxy.borrow_mut().ticket_as_header()
        .map_err(CombinedError::CocaineError)
        .and_then(|header| {
            println!("subscribing to path: {}", path);
            kids_subscribe(
                Service::new("unicorn", &handle),
                path,
                make_auth_headers(header),
                tx
            )
        });

    let path = String::from(path);
    let spawn_handle = core.handle();
    let node_handler = core.handle();

    let nodes_future = rx.for_each(move |(version, nodes)| {
        println!("got from queue {} item(s) with version {}", nodes.len(), version);

        // for node in &nodes {
        //     println!("\t{} {:?}", path, node);
        // }

        let proxy = Rc::clone(&proxy);
        let cluster = Arc::clone(&cluster);
        let node_handler = node_handler.clone();
        let path = path.clone();

        let processing_future = proxy.borrow_mut().ticket_as_header()
            .map_err(CombinedError::CocaineError)
            .and_then(move |header| {
                let mut results = Vec::with_capacity(nodes.len());

                let handle = node_handler.clone();

                for uuid in nodes {
                    let auth_hdr = make_auth_headers(header.clone());
                    let node_path = format!("{}/{}", path.clone(), uuid);

                    let ft = get_node::<_, NodeInfo>(Service::new("unicorn", &handle), auth_hdr, &node_path)
                        .and_then(move |data| Ok((uuid, data)));

                    results.push(ft);
                }

                future::join_all(results)
            })
            .and_then(move |nodes| {
                let mut cls = cluster.write().unwrap();
                cls.update(nodes);
                Ok(())
            })
            .then(|result| match result {
                // TODO: print timestamp
                Ok(_) => { println!("state has been updated"); Ok(()) },
                Err(err) => { println!("Error {:?}", err); Ok(()) }
            });

        spawn_handle.spawn(processing_future);
        Ok(())
    });

    core.handle().spawn(nodes_future);
    match core.run(subscibe_future) {
        Ok(_) => println!("subcription processing done"),
        Err(err) => println!("Got an error {:?}", err)
    }
}

fn make_requests<C>(cli: hyper::Client<C>, uuid: String, ep: Endpoint, handle: Handle)
    -> Box<Future<Item=(), Error=CombinedError>>
where
    C: hyper::client::Connect
{
    fn ip6_uri_from_string(uri: &str, port: u16, path: &str)
        -> Result<hyper::Uri, hyper::error::UriError>
    {
        let uri = format!("[{}]:{}/{}", uri, port, path);
        uri.parse::<hyper::Uri>()
    }

    fn get<'a,T,C>(cli: &'a hyper::Client<C>, uri: hyper::Uri)
        -> Box<Future<Item=T, Error=CombinedError> + 'a>
    where
        C: hyper::client::Connect,
        T: serde::de::DeserializeOwned + 'a
    {
        let data = cli.get(uri)
            .and_then(|res| {
                res.body().fold(Vec::new(), |mut acc, chunk| {
                    acc.extend(&chunk[..]);
                    future::ok::<Vec<u8>,hyper::Error>(acc)
                })
            })
            .map_err(CombinedError::HyperError)
            .and_then(|raw| {
                match serde_json::from_slice::<T>(&raw) {
                    Ok(d) => future::ok::<T,_>(d),
                    Err(e) => future::err(CombinedError::SerdeError(e))
                }
            });

        Box::new(data)
    }

    let info_uri = ip6_uri_from_string(&ep.host_str(), DEFAULT_WEB_PORT, "info");
    let state_uri = ip6_uri_from_string(&ep.host_str(), DEFAULT_WEB_PORT, "state");
    let _metrics_uri = ip6_uri_from_string(&ep.host_str(), DEFAULT_WEB_PORT, "metrics");

    let info_future = info_uri.map(|uri| {
        get::<orca::Info, C>(&cli, uri)
            .and_then(|_info| Ok(())) // TODO: process info record
    });

    let state_future = state_uri.map(|uri| {
        get::<orca::CommitedState, C>(&cli, uri)
            .and_then(|_info| Ok(())) // TODO: process state record
    });

    // let f = state_future.join(info_future);
    // Box::new(state_future.join(info_future))
    Box::new(future::ok(()))
}


fn _gather_info(core: Core, cluster: Arc<SyncedCluster>, _orcas: Arc<SyncedOrcasPod>)
 -> Box<Future<Item=Vec<()>, Error=CombinedError>>
{

    fn is_ipv6(addr: &Endpoint) -> bool {
        match net::IpAddr::from_str(&addr.host_str()) {
            Ok(addr) => addr.is_ipv6(),
            _ => false
        }
    }

    let mut gather_strides = Vec::with_capacity(cluster.read().unwrap().len());
    let endpoints = cluster.read().unwrap().endpoints();

    println!("cluster size {}", endpoints.len());

    // let core = Core::new().unwrap();
    // let cli = hyper::Client::new(&core.handle());

    for (num, (uuid, ep)) in endpoints.iter().enumerate() {
        let to_sleep = time::Duration::new((num as u64 % GATHER_INTERVAL_SEC) * 1000, 0);
        let handle = core.handle();

        let gather_bootstrap = Timeout::new(to_sleep, &core.handle())
            .map_err(CombinedError::IOError)
            .and_then(move |_| Ok((uuid, ep)))
            .and_then(|(uuid, eps)| {
                let eps_v6: Vec<_> = eps.iter().filter(|ep| is_ipv6(ep)).collect();

                //
                // TODO: make one client for all requests.
                // TODO: first address taken (if any), but should take random one
                //
                let cli = hyper::Client::new(&core.handle());
                if let Some(ref ep) = eps_v6.first() {
                    let requests = make_requests(cli, uuid.to_string(), (**ep).clone(), handle);
                    Ok(requests)
                } else {
                    let error_message = format!("can't find ip6 address for uuid {} within hosts {:?}", uuid, eps);
                    Err(CombinedError::Other(error_message))
                }
            });

        gather_strides.push(gather_bootstrap);
    }

    // Box::new(future::join_all(gather_strides))
    let r = Vec::new();
    Box::new(future::ok(r))
}


pub fn gather_info(cluster: Arc<SyncedCluster>, _orcas: Arc<SyncedOrcasPod>) {
    let cls = cluster.read().unwrap();
    println!("cluster size {}", cls.len());
}
