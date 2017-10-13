extern crate cocaine;
extern crate futures;

#[macro_use]
extern crate serde_derive;
extern crate clap;

use clap::{App, Arg};

mod samples;

use samples::{
    read_from_storage,
    read_from_unicron,
    listen_unicorn
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

    read_from_storage("store", "test.txt");
    if let Some(path) = options.value_of("unicorn_path") {
        read_from_unicron(path);
        listen_unicorn(path);
    }
}
