#![feature(conservative_impl_trait)]

extern crate cocaine;
extern crate futures;
extern crate tokio_core;

#[macro_use]
extern crate serde_derive;
extern crate clap;

use clap::{App, Arg};

use tokio_core::reactor::Core;

mod samples;

use samples::{
    read_from_unicron,
    unicorn_subscribe
};

fn main() {
    let options = App::new("Cocaine test app")
        .version("unreleased")
        .arg(Arg::with_name("unicorn_path")
            .short("p")
            .long("path")
            .takes_value(true)
            .help("path to listen unicorn events on"))
        .get_matches();

    // read_from_storage("store", "test.txt");
    if let Some(path) = options.value_of("unicorn_path") {
        println!("Starting unicorn patch");
        // read_from_unicron(path);

        let mut core = Core::new().unwrap();
        let future = unicorn_subscribe("unicorn", path, &core.handle());

        core.run(future).unwrap();

        println!("listen done");
    }
}
