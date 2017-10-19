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

use cocaine::Service;

use clap::{App, Arg};

use tokio_core::reactor::Core;
use futures::Future;

mod samples;
mod config;
mod secure;


use samples::{
    // read_from_unicron,
    unicorn_subscribe,
    unicorn_kids_subscribe,
    unicorn_get_node
};

use config::Config;

use secure::TvmProxy;


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
    println!("get config {:?}", config);
    println!("is ticket option present {}", options.is_present("ticket"));

    if let Some(path) = options.value_of("unicorn_path") {
        println!("Starting unicorn patch");
        let mut core = Core::new().unwrap();

        let future1 = unicorn_subscribe("unicorn", path, &core.handle());

        if let Some(node) = options.value_of("node") {
            let future2 = unicorn_get_node("unicorn", node, &core.handle());
            core.run(future1.join(future2)).unwrap();
        } else {
            core.run(future1).unwrap();
        }
    } else if let Some(path) = options.value_of("kids_path") {
        let mut core = Core::new().unwrap();
        let future = unicorn_kids_subscribe("unicorn", path, &core.handle());
        core.run(future).unwrap();
    } else if options.is_present("ticket") {
        let mut core = Core::new().unwrap();
        let tvm = Service::new("tvm", &core.handle());
        println!("creating proxy");
        if let Ok(mut proxy) = TvmProxy::new(&config, tvm) {
            println!("trying to fecth ticket...");
            if let Ok(ticket) = core.run(proxy.ticket()) {
                println!("got it {}", ticket);
            }
        }
    }
}
