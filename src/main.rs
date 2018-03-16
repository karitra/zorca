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

#[macro_use] extern crate clap;
extern crate time;

extern crate hyper;
extern crate hyper_staticfile;

use clap::{App, Arg, ArgMatches};
use std::sync::Arc;
use std::net::SocketAddr;

use futures::Stream;

use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;

use hyper::server::Http;

use cocaine::Service;
use cocaine::service::Unicorn;

mod samples;
mod config;
mod secure;
mod errors;
mod unicorn;
mod engine;
mod orca;
mod resources;
mod web;


use engine::{
    Cluster,
    SyncedCluster,
    subscription,
    gather,
};

use orca::{
    SyncedApps,
    SyncedOrcasPod,
    OrcasPod,
    AppsFilterType,
    AppsTrait,
};

use web::{WebApi, SelfInfo};

use samples::make_dummy_cluster;


const SUSPEND_DURATION_SEC: u64 = 10;
const POLL_DURATION_SEC: u64 = 10;


// TODO: make separate inteface
struct Context<'a> {
    config: config::Config,
    options: ArgMatches<'a>
}

impl<'a> Context<'a> {
    fn get_listen_path(&self) -> &str {
        self.options.value_of("kids_path").unwrap()
    }
}


fn main() {
    let options = App::new("Cocaine orchestrator(s) monitoring tools")
        .version(crate_version!())
        .arg(Arg::with_name("kids_path")
            .short("k")
            .long("kids")
            .required(true)
            .takes_value(true)
            .help("node to subscribe for kids updates"))
        .arg(Arg::with_name("dummy_data")
            .short("d")
            .long("dummy")
            .help("use dummy host data for testing and debuging"))
        .arg(Arg::with_name("gather_interval")
            .short("i")
            .long("interval")
            .takes_value(true)
            .help("orca stats gather interval"))
        .get_matches();

    //
    // Make setup and config processing
    //
    let mut config = config::Config::new_from_default_files();
    config.gather_interval = value_t!(options, "gather_interval", u64)
        .unwrap_or(config::GATHER_INTERVAL_SECS);

    if config.gather_interval == 0 {
        config.gather_interval = config::GATHER_INTERVAL_SECS;
        println!("gather_interval can't be zero, reset to {}", config.gather_interval);
    }

    let context = Arc::new(Context{config, options});

    //
    // TODO: factory for hide construction details?
    //
    let cluster = Arc::new(SyncedCluster::new(Cluster::new()));
    let orcas = Arc::new(SyncedOrcasPod::new(OrcasPod::new()));
    let apps = Arc::new(SyncedApps::new(orca::Apps::new()));
    let apps_mismatched = Arc::new(SyncedApps::new(orca::Apps::new()));

    let ctx_for_subscribe = Arc::clone(&context);
    let cluster_for_subscribe = Arc::clone(&cluster);

    std::thread::spawn(move || {

        loop {
            let mut core = Core::new().unwrap();
            let unicorn = Unicorn::new(Service::new("unicorn", &core.handle()));

            let cls = Arc::clone(&cluster_for_subscribe);
            let cls1 = Arc::clone(&cluster_for_subscribe);

            // TODO: Cocaine RT (unicorn) endpoints
            let work = subscription(
                &unicorn,
                core.handle(),
                &ctx_for_subscribe.config,
                ctx_for_subscribe.get_listen_path(),
                cls,
            );

            match core.run(work) {
                // TODO: timestamp
                Ok(_) => println!("cluster info updated"),
                Err(e) => println!("error while obtaining cluster state {:?}", e)
            };

            cls1.write().unwrap().clear();

            // sleep on subscribe error and try again
            std::thread::sleep(std::time::Duration::new(SUSPEND_DURATION_SEC, 0));
        }
    });

    let cluster_for_gather = match context.options.is_present("dummy_data") {
        true => Arc::new(SyncedCluster::new(make_dummy_cluster())),
        false => Arc::clone(&cluster)
    };

    let ctx_for_gather = Arc::clone(&context);
    let orcas_for_gather = Arc::clone(&orcas);
    let apps_for_gather = Arc::clone(&apps);
    let apps_mismatched_for_gather = Arc::clone(&apps_mismatched);

    std::thread::spawn(move || {

        println!("gather interval: {}", ctx_for_gather.config.gather_interval);
        let gather_interval = ctx_for_gather.config.gather_interval;

        loop {
            let mut core = Core::new().unwrap();
            let client = hyper::client::Client::new(&core.handle());

            let work = gather(
                &client,
                Arc::clone(&cluster_for_gather),
                Arc::clone(&orcas_for_gather),
                gather_interval,
            );

            match core.run(work) {
                Ok(_) => println!("orcas pod has been updated"),
                Err(e) => println!("failed to request orcas with error {:?}", e),
            };

            let len = orcas_for_gather.read().unwrap().len();
            println!("orcas pod size now is {}", len);

            {   // Update apps stat.
                let orcas = orcas_for_gather.read().unwrap();
                apps_for_gather.write().unwrap().update(&orcas, AppsFilterType::GenerateAll);
                apps_mismatched_for_gather.write().unwrap().update(&orcas, AppsFilterType::GenerateMismatch);
            }

            let len = apps_for_gather.read().unwrap().len();
            println!("apps in global state {}", len);

            std::thread::sleep(std::time::Duration::new(POLL_DURATION_SEC, 0));
        }
    });

    let self_info = SelfInfo::new(crate_version!());

    let model = web::Model {
        cluster: Arc::clone(&cluster),
        orcas: Arc::clone(&orcas),
        apps: Arc::clone(&apps),
        apps_mismatched: Arc::clone(&apps_mismatched),
        self_info
    };

    loop {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        // TODO: take from config
        let address: SocketAddr = "[::1]:3141".parse().unwrap();
        let listener = TcpListener::bind(&address, &handle).unwrap();

        // TODO: hide details somehow.
        let http = Http::new();
        let server = listener.incoming().for_each(|(sock, addr)| {
            // TODO: static file folder from config.
            let web = WebApi::new(&handle, model.clone(), "assets");
            http.bind_connection(&handle, sock, addr, web);
            Ok(())
        });

        match core.run(server) {
            Ok(_) => println!("web service exited normally"),
            Err(e) => println!("error in web service {:?}", e)
        };

        std::thread::sleep(std::time::Duration::new(SUSPEND_DURATION_SEC, 0));
    }
}
