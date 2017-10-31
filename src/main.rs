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


use clap::{App, Arg, ArgMatches};
use std::sync::Arc;


mod samples;
mod config;
mod secure;
mod types;
mod errors;
mod unicorn;
mod engine;
mod resources;

use config::Config;
use engine::{
    Cluster,
    subscription
};


const SUSPEND_DURATION: u64 = 10;


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
        .version("unreleased")
        .arg(Arg::with_name("kids_path")
            .short("k")
            .long("kids")
            .required(true)
            .takes_value(true)
            .help("node to subscribe for kids updates"))
        .get_matches();

    let config = Config::new_from_default_files();
    let context = Arc::new(Context{config, options});

    let ctx_for_subscribe = Arc::clone(&context);
    let subscribe_thread = std::thread::spawn(move || {
        loop {
            subscription(&ctx_for_subscribe.config, ctx_for_subscribe.get_listen_path());
            std::thread::sleep(std::time::Duration::new(SUSPEND_DURATION, 0));
        }
    });

    subscribe_thread.join().unwrap();
}
