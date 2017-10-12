extern crate cocaine;
extern crate futures;

use cocaine::{Core, Service};
use cocaine::service::Storage;

fn read_from_storage(collection: &str, key: &str) {
    let mut core = Core::new().unwrap();
    let storage = Storage::new(Service::new("storage", &core.handle()));

    println!("collection: {} key: {}", collection, key);

    let future = storage.read(collection, key);

    let data = core.run(future).unwrap();
    // print!("This is data: {:?}", data);
}

fn main() {
    println!("Hello, world!");
    read_from_storage("store", "test.txt");
}
