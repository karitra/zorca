extern crate clap;
extern crate cocaine;
extern crate futures;
#[macro_use]
extern crate serde_derive;

use clap::{App, Arg};
use cocaine::{Core, Service};
use cocaine::service::{Storage, Unicorn};

use std::collections::HashMap;

#[derive(Deserialize, Debug)]
struct StateRecord {
    workers: i32,
    profile: String
}

type State = HashMap<String, StateRecord>;


fn read_from_unicron(path: &str) {
    println!("path: {}", path);

    let mut core = Core::new().unwrap();
    let unicorn = Unicorn::new(Service::new("unicorn", &core.handle()));

    let future = unicorn.get::<State>(path);

    match core.run(future) {
        Ok((Some(data), version)) =>
            println!("version: {}, data: {:?}", version, data),
        Ok((None, _)) => println!("no data"),
        Err(error) => println!("error: {:?}", error),
    }
}

fn read_from_storage(collection: &str, key: &str) {
    let mut core = Core::new().unwrap();
    let storage = Storage::new(Service::new("storage", &core.handle()));

    println!("collection: {} key: {}", collection, key);

    let future = storage.read(collection, key);

    match core.run(future) {
        Ok(data) => println!("data: {:?}", data),
        Err(error) => println!("error: {:?}", error),
    }
}

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
    }
}
