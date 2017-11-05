#![feature(conservative_impl_trait)]
#![feature(underscore_lifetimes)]
//
// TODO: integrate clap and yaml
//
extern crate cocaine;
extern crate futures;
extern crate tokio_core;


#[macro_use] extern crate serde_derive;
extern crate serde;
extern crate serde_json;

extern crate yaml_rust;

extern crate clap;
extern crate time;
extern crate hyper;


use clap::{App, Arg, ArgMatches};
use std::sync::Arc;


mod samples;
mod config;
mod secure;
mod types;
mod errors;
mod unicorn;
mod engine;
mod orca;
mod resources;

use config::Config;
use engine::{
    Cluster,
    SyncedCluster,
    subscription,
    gather_info,
};

use orca::{
    SyncedOrcasPod,
    OrcasPod
};


const SUSPEND_DURATION_SEC: u64 = 10;
const POLL_DURATION_SEC: u64 = 10;


struct Context<'a> {
    config: Config,
    options: ArgMatches<'a>
}

impl<'a> Context<'a> {
    fn get_listen_path(&self) -> &str {
        self.options.value_of("kids_path").unwrap()
    }
}


fn main() {
    let options = App::new("Cocaine orchestrator(s) monitoring tools")
        .version("unreleased-1")
        .arg(Arg::with_name("kids_path")
            .short("k")
            .long("kids")
            .required(true)
            .takes_value(true)
            .help("node to subscribe for kids updates"))
        .get_matches();

    let config = Config::new_from_default_files();
    let context = Arc::new(Context{config, options});

    // TODO: factory for hide construction details
    let cluster = Arc::new(SyncedCluster::new(Cluster::new()));
    let orcas = Arc::new(SyncedOrcasPod::new(OrcasPod::new()));

    let ctx_for_subscribe = Arc::clone(&context);
    let cluster_for_subscribe = Arc::clone(&cluster);
    let subscribe_thread = std::thread::spawn(move || {
        let cls = cluster_for_subscribe;
        loop {
            let cls = Arc::clone(&cls);
            let cls1 = Arc::clone(&cls);

            subscription(
                &ctx_for_subscribe.config,
                ctx_for_subscribe.get_listen_path(),
                cls,
            );

            {
                // We are in case of Cocaine error here, reset cluster info.
                let mut cls = cls1.write().unwrap();
                cls.clear();
            }

            // sleep on subscribe error and try again
            std::thread::sleep(std::time::Duration::new(SUSPEND_DURATION_SEC, 0));
        }
    });

    let _ctx_for_gather = Arc::clone(&context);
    let cluster_for_gather = Arc::clone(&cluster);
    let state_gather_thread = std::thread::spawn(move || {
        let cls = cluster_for_gather;
        let orcas = orcas;
        loop {
            let cls = Arc::clone(&cls);
            let orcas = Arc::clone(&orcas);
            gather_info(cls, orcas);
            std::thread::sleep(std::time::Duration::new(POLL_DURATION_SEC, 0));
        }
    });

    state_gather_thread.join().unwrap();
    subscribe_thread.join().unwrap();
}
