use cocaine::Service;
use cocaine::hpack::RawHeader;

use tokio_core::reactor::Core;

use futures::future;
use futures::{Future, Stream};
use futures::sync::mpsc;

use std::rc::Rc;
use std::cell::RefCell;

use std::collections::HashMap;

use secure::make_ticket_service;
use errors::CombinedError;
use types::SubscribeMessage;
use config::Config;
use resources::NodeInfo;

use unicorn::{
    kids_subscribe,
    get_node
};


pub type Cluster = HashMap<String, NodeInfo>;
type AuthHeaders = Vec<RawHeader>;


fn make_auth_headers(header: Option<String>) -> Option<AuthHeaders>  {
    header.and_then(|hdr| {
        let hdrs = vec![
            RawHeader::new("authorization".as_bytes(), hdr.into_bytes())
        ];
        Some(hdrs)
    })
}

fn update_state(nodes: Vec<Option<NodeInfo>>) -> Box<Future<Item=(), Error=CombinedError>> {
    for nd in nodes {
        if let Some(nd) = nd {
            println!("node: {:?}", nd);
        }
    }

    let r = future::ok::<(), CombinedError>(());
    Box::new(r)
}

pub fn subscription<'a>(config: &Config, path: &'a str) {
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
        for node in &nodes {
            println!("\t{} {:?}", path, node);
        }

        let proxy = Rc::clone(&proxy);
        let node_handler = node_handler.clone();
        let path = path.clone();

        let processing_future = proxy.borrow_mut().ticket_as_header()
            .map_err(CombinedError::CocaineError)
            .and_then(move |header| {
                let mut results = Vec::with_capacity(nodes.len());

                let handle = node_handler.clone();

                for node in nodes {
                    let auth_hdr = make_auth_headers(header.clone());
                    let node_path = format!("{}/{}", path.clone(), node);

                    // println!("requesting node: {:?}", node_path);

                    let ft = get_node::<_, NodeInfo>(Service::new("unicorn", &handle), auth_hdr, &node_path);
                    results.push(ft);
                }

                future::join_all(results)
            })
            .and_then(|nodes| update_state(nodes))
            .then(|result| match result {
                // TODO: print timestamp
                Ok(v) => { println!("state has been updated {:?}", v); Ok(()) },
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
