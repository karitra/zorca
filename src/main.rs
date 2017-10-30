#![feature(conservative_impl_trait)]
//
// TODO: integrate clap and yaml
//
extern crate cocaine;
extern crate futures;
extern crate tokio_core;

#[macro_use] extern crate serde_derive;

extern crate yaml_rust;

extern crate clap;
extern crate time;
extern crate serde;

use cocaine::{Service, Error};
use cocaine::hpack::RawHeader;

use clap::{App, Arg};

use tokio_core::reactor::Core;

use futures::Future;
use futures::sync::mpsc;
use futures::Stream;


mod samples;
mod config;
mod secure;


use samples::{
    // read_from_unicron,
    unicorn_subscribe,
    unicorn_kids_subscribe,
    unicorn_get_node,
    State
};

use secure::make_ticket_service;

use config::Config;


fn main() {
    let options = App::new("Cocaine test app")
        .version("unreleased")
        .arg(Arg::with_name("unicorn_path")
            .short("p")
            .long("path")
            .takes_value(true)
            .help("path to listen unicorn events on"))
        .arg(Arg::with_name("node")
            .short("n")
            .long("node")
            .takes_value(true)
            .help("node to read value from"))
        .arg(Arg::with_name("kids_path")
            .short("k")
            .long("kids")
            .takes_value(true)
            .help("node to subscribe for kids update"))
        .arg(Arg::with_name("ticket")
            .short("t")
            .long("ticket")
            .help("get a `ticket to ride`"))
        .get_matches();

    let config = Config::new_from_default_files();

    if let Some(path) = options.value_of("unicorn_path") {
        println!("Starting unicorn patch");
        let mut core = Core::new().unwrap();

        let future1 = unicorn_subscribe::<State>("unicorn", path, &core.handle());

        if let Some(node) = options.value_of("node") {
            let future2 = unicorn_get_node::<String>("unicorn", node, &core.handle());
            core.run(future1.join(future2)).unwrap();
        } else {
            core.run(future1).unwrap();
        }
    }

    if options.is_present("ticket") {
        let mut core = Core::new().unwrap();

        println!("creating proxy");

        let mut proxy = make_ticket_service(Service::new("tvm", &core.handle()), &config);

        if let Some(path) = options.value_of("kids_path") {
            type Msg = (String, Vec<String>);
            let (tx, rx) = mpsc::unbounded::<Msg>();

            let handle = core.handle();

            let subscibe_future = proxy.ticket_as_header().and_then(|header| {

                let headers = header.and_then(|hdr| {
                    let hdrs = vec![
                        RawHeader::new("authorization".as_bytes(), hdr.into_bytes())
                    ];
                    Some(hdrs)
                });

                println!("subscribing to path: {}", path);
                unicorn_kids_subscribe(Service::new("unicorn", &handle), path, headers, handle, tx)
            });

            let nodes_future = rx.for_each(|nodes| {
                println!("got from queue {} item(s)", nodes.1.len());
                Ok(())
            });

            core.handle().spawn(nodes_future);
            match core.run(subscibe_future) {
                Ok(_) => println!("processing done"),
                Err(err) => println!("Got an error {:?}", err)
            };
        }
    }
}
