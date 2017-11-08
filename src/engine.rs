use cocaine::Service;
use cocaine::hpack::RawHeader;

use tokio_core::reactor::{Handle, Timeout};

use hyper;

use futures::future;
use futures::{Future, Stream};
use futures::sync::mpsc;

use serde;
use serde_json;

use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, RwLock};
use std::str::FromStr;
use std::time::{self, UNIX_EPOCH};
use std::iter::Iterator;
use std::net;
use std::collections::HashMap;

use secure::make_ticket_service;
use errors::CombinedError;
use config::Config;
use resources::{Endpoint, NodeInfo};

use unicorn::{
    kids_subscribe,
    get_node
};


use orca;
use orca::{
    OrcaRecord,
    SyncedOrcasPod,
    DEFAULT_WEB_PORT
};


// Note: in case of massive cluster updates (score of machines was restarted),
//       it could be quite massive subscription update rate, so channel queue size
//       can help hold mem usage constrained in that case or in case when unicorn
//       will go nuts and flood ecosystem with subscriptions.
const SUBSCRIBE_QUEUE_SIZE: usize = 1024;

// TODO: Should be in config with reasonable defautls someday.
const GATHER_INTERVAL_SECS: u64 = 120;
const ONE_HOUR_IN_SECS: u64 = 1 * 60 * 60;


pub type SubscribeMessage = (i64, Vec<String>);

pub type Cluster = HashMap<String, NodeInfo>;
pub type SyncedCluster = RwLock<Cluster>;

type AuthHeaders = Vec<RawHeader>;
type UuidNodeInfo = (String, Option<NodeInfo>);


#[derive(Debug, Clone)]
pub struct NetInfo {
    hostname: String,
    endpoints: Vec<Endpoint>
}

trait ClusterInterface {
    fn update(&mut self, nodes: Vec<UuidNodeInfo>);
    fn hosts(&self) -> HashMap<String, NetInfo>;
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

    fn hosts(&self) -> HashMap<String, NetInfo> {
        let mut endpoints: HashMap<String, NetInfo> = HashMap::with_capacity(self.len());
        for (uuid, node_info) in self {
            let net = NetInfo {
                hostname: node_info.hostname.clone(),
                endpoints: node_info.endpoints.clone()
            };
            endpoints.entry(uuid.clone()).or_insert(net);
        }
        endpoints
    }
}


fn make_auth_headers(header: Option<String>) -> Option<AuthHeaders>  {
    header.and_then(|hdr|
        Some(vec![ RawHeader::new("authorization".as_bytes(), hdr.into_bytes()) ])
    )
}

pub fn subscription<'a>(handle: Handle, config: &Config, path: &'a str, cluster: Arc<SyncedCluster>)
    -> Box<Future<Item=(), Error=CombinedError> + 'a>
{
    let proxy = make_ticket_service(Service::new("tvm", &handle), &config);
    let proxy = Rc::new(RefCell::new(proxy));

    let (tx, rx) = mpsc::channel::<SubscribeMessage>(SUBSCRIBE_QUEUE_SIZE);

    let subscribe_handle = handle.clone();
    let subscribe_path = String::from(path);

    let subscibe_future = proxy.borrow_mut().ticket_as_header()
        .map_err(CombinedError::CocaineError)
        .and_then(move |header| {
            println!("subscribing to path: {}", subscribe_path);
            kids_subscribe(
                Service::new("unicorn", &subscribe_handle),
                subscribe_path,
                make_auth_headers(header),
                tx
            )
        });

    let path = String::from(path);
    let spawn_handle = handle.clone();
    let node_handler = handle.clone();

    let nodes_future = rx.for_each(move |(version, nodes)| {
        println!("got from queue {} item(s) with version {}", nodes.len(), version);

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

    handle.spawn(nodes_future);
    Box::new(subscibe_future)
}


type OrcaRequestResult = (String, orca::Orca); // (hostname, orca)

fn make_requests<'a, C>(
    client: &'a hyper::client::Client<C>, endpoint: Endpoint, net_info: &NetInfo)
    -> Box<Future<Item=OrcaRequestResult, Error=CombinedError> + 'a>
where
    C: hyper::client::Connect + 'a
{
    fn ip6_uri_from_string(uri: &str, port: u16, path: &str)
        -> Result<hyper::Uri, hyper::error::UriError>
    {
        let uri = format!("{}://[{}]:{}/{}", orca::DEFAULT_WEB_SCHEME, uri, port, path);
        uri.parse::<hyper::Uri>()
    }

    // TODO: make connector pluggable
    fn get<'a,C,T>(cli: &'a hyper::Client<C>, uri: hyper::Uri)
        -> Box<Future<Item=T, Error=CombinedError> + 'a>
    where
        C: hyper::client::Connect + 'a,
        T: serde::de::DeserializeOwned + 'a
    {
        // println!("get for {:?}", uri);
        let data = cli.get(uri)
            .and_then(|res| {
                // println!("result {}", res.status());
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

    let info_uri = ip6_uri_from_string(&endpoint.host_str(), DEFAULT_WEB_PORT, "info");
    let state_uri = ip6_uri_from_string(&endpoint.host_str(), DEFAULT_WEB_PORT, "state");
    let _metrics_uri = ip6_uri_from_string(&endpoint.host_str(), DEFAULT_WEB_PORT, "metrics");

    let info_future = future::result(info_uri)
        .map_err(CombinedError::UriParseError)
        .and_then(|uri| { // TODO: Debug clusure, remove
            // println!("making info request for {:?}", uri);
            Ok(uri)
        })
        .and_then(move |uri| get::<C, orca::Info>(client, uri));

    let state_future = future::result(state_uri)
        .map_err(CombinedError::UriParseError)
        .and_then(|uri| { // TODO: Debug clusure, remove
            // println!("making state request {:?}", uri);
            Ok(uri)
        })
        .and_then(move |uri| get::<C, orca::CommitedState>(client, uri));

    let hostname = net_info.hostname.clone();
    let endpoint = endpoint.clone();

    let request_result = info_future.join(state_future)
        .then(move |r| match r {
            Ok((info, state)) => {
                let orca = orca::Orca {
                    endpoints: vec![ endpoint ],
                    state,
                    info,
                };
                Ok((hostname, orca))
            },
            Err(e) => Err(e)
        });

    Box::new(request_result)
}


#[allow(dead_code)]
fn dump_cls(cls: &Cluster) {
    for (uuid, node) in cls {
        println!("{} {}", uuid, node.hostname);
    }
}


pub fn gather<'a,C>(client: &'a hyper::client::Client<C>, cluster: Arc<SyncedCluster>, orcas: Arc<SyncedOrcasPod>)
    -> Box<Future<Item=(), Error=CombinedError> + 'a>
where
    C: hyper::client::Connect + 'a
{

    fn is_ipv6(addr: &Endpoint) -> bool {
        match net::IpAddr::from_str(&addr.host_str()) {
            Ok(addr) => addr.is_ipv6(),
            _ => false
        }
    }

    let hosts = cluster.read().unwrap().hosts();
    let mut gather_strides = Vec::with_capacity(cluster.read().unwrap().len());

    println!("cluster size {}", hosts.len());

    for (num, (uuid, net)) in hosts.iter().enumerate() {

        let to_sleep = time::Duration::new(num as u64 % GATHER_INTERVAL_SECS, 0);
        // println!("timeout {:?}", (num as u64 % GATHER_INTERVAL_SECS));

        let gather_bootstrap = match Timeout::new(to_sleep, &client.handle()) {
            Ok(_timout) => {
                let eps_v6: Vec<_> = net.endpoints.iter().filter(|net| is_ipv6(net)).collect();

                // println!("uuid {}", uuid);

                //
                // TODO: first address taken (if any), but should we peek a random one?
                //
                match eps_v6.first() {
                    Some(ref ep) => make_requests(client, (**ep).clone(), net),
                    None => {
                        let error_message = format!("can't find ip6 address for uuid {} within host {:?}", uuid, net);
                        Box::new(future::err(CombinedError::Other(error_message)))
                    }
                }
            },

            // Propogate timeout error as a future.
            Err(e) => Box::new(future::err(CombinedError::IOError(e)))
        };

        let completion = gather_bootstrap
            .then(|r| match r {
                Ok(r) => Ok(Some(r)),
                //
                // TODO: For now error is ignored silently, but we should
                //       report it to some kind of logger someday.
                Err(_e) => {
                    // println!("error on request {:?}", e);
                    Ok(None)
                }
            });

        gather_strides.push(completion);
    }

    let result = future::join_all(gather_strides)
        .and_then(move |responses| {

            let now = time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            let span = time::Duration::from_secs(ONE_HOUR_IN_SECS);

            { // Remove old records.
                let mut orcas = orcas.write().unwrap();
                orcas.retain(|_host, orca|
                    now - time::Duration::from_secs(orca.update_timestamp) < span);
            }

            {
                let mut orcas = orcas.write().unwrap();
                for val in responses {
                    if let Some((host, orca)) = val {
                        let record = OrcaRecord { orca, update_timestamp: now.as_secs() };
                        orcas.insert(host, record);
                    }
                }
            }

            Ok(())
        });

    Box::new(result)
}
