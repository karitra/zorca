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


const GATHER_INTERVAL_SEC: u64 = 2;


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


pub fn subscription<'a>(handle: Handle, config: &Config, path: &'a str, cluster: Arc<SyncedCluster>)
    -> Box<Future<Item=(), Error=CombinedError> + 'a>
{
    let proxy = make_ticket_service(Service::new("tvm", &handle), &config);
    let proxy = Rc::new(RefCell::new(proxy));

    let (tx, rx) = mpsc::unbounded::<SubscribeMessage>();

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

    handle.spawn(nodes_future);
    Box::new(subscibe_future)
}

fn make_requests<C>(cli: Rc<hyper::Client<C>>, uuid: String, ep: Endpoint)
    -> Box<Future<Item=(), Error=CombinedError>>
where
    C: hyper::client::Connect
{
    fn ip6_uri_from_string(uri: &str, port: u16, path: &str)
        -> Result<hyper::Uri, hyper::error::UriError>
    {
        let uri = format!("{}://[{}]:{}/{}", orca::DEFAULT_WEB_SCHEME, uri, port, path);
        println!("uri: {:?}", uri);
        uri.parse::<hyper::Uri>()
    }

    fn get<'a,T,C>(cli: &hyper::Client<C>, uri: hyper::Uri)
        -> Box<Future<Item=T, Error=CombinedError> + 'a>
    where
        C: hyper::client::Connect,
        T: serde::de::DeserializeOwned + 'a
    {
        println!("get for {:?}", uri);
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

    let _uuid = Rc::new(uuid);

    let cli_for_info = Rc::clone(&cli);
    let info_future = future::result(info_uri)
        .and_then(|uri| {
            println!("ready to connect to uri {:?}", uri);
            Ok(uri)
        })
        .map_err(CombinedError::UriParseError)
        .and_then(|uri| { // TODO: Debug clusure, remove
            println!("making info request for {:?}", uri);
            Ok(uri)
        })
        .and_then(move |uri| get::<orca::Info, C>(&cli_for_info, uri))
        .and_then(|info| {
            println!("info {:?}", info);
            Ok(())
        });

    let cli_for_state = Rc::clone(&cli);
    let state_future = future::result(state_uri)
        .map_err(CombinedError::UriParseError)
        .and_then(|uri| { // TODO: Debug clusure, remove
            println!("making state request {:?}", uri);
            Ok(uri)
        })
        .and_then(move |uri| get::<orca::CommitedState, C>(&cli_for_state, uri))
        .and_then(|state| {
            println!("state {:?}", state);
            Ok(())
        });

    //
    // let r = Box::new(state_future.join(info_future).then(|r|
    //     match r {
    //         Ok(v) => {println!("it seems make request done {:?}", v); future::ok(())},
    //         Err(e) => {println!("error {:?}", e); future::err(e)}
    //     }
    // ));
    //
    // println!("result is {:?}", r);
    //

    Box::new(state_future.join(info_future).then(|r| {
        println!("request completion {:?}", r);
        Ok(())
    }))
}


// TODO: current implemetation will cancel request in case of error in any one,
//       reimplement to make complete other in case of some tasks fail.
pub fn gather(handle: Handle, cluster: Arc<SyncedCluster>, _orcas: Arc<SyncedOrcasPod>)
 -> Box<Future<Item=(), Error=CombinedError>>
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

    let cli_handle = handle.clone();
    let cli = Rc::new(hyper::Client::new(&cli_handle));

    let handle = Rc::new(handle);

    for (num, (uuid, ep)) in endpoints.iter().enumerate() {
        let to_sleep = time::Duration::new((num as u64 % GATHER_INTERVAL_SEC), 0);

        let cli = Rc::clone(&cli);
        let gather_bootstrap = Timeout::new(to_sleep, &handle)
            .map_err(CombinedError::IOError)
            .and_then(move |_| Ok((uuid, ep)))
            .and_then(|(uuid, eps)| {
                let eps_v6: Vec<_> = eps.iter().filter(|ep| is_ipv6(ep)).collect();

                //
                // TODO: first address taken (if any), but should we peek random one
                //
                match eps_v6.first() {
                    Some(ref ep) => {
                        let requests = make_requests(cli, uuid.to_string(), (**ep).clone());
                        println!("preparing request for {}: {:?}", uuid, ep);
                        Ok(requests)
                    },
                    None => {
                        let error_message = format!("can't find ip6 address for uuid {} within hosts {:?}", uuid, eps);
                        Err(CombinedError::Other(error_message))
                    }
                }
            });

        gather_strides.push(gather_bootstrap);
    }

    Box::new(future::join_all(gather_strides).and_then(|_| Ok(())))
}
