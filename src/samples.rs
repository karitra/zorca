use cocaine::{Core, Error, Service};
use cocaine::service::{Storage, Unicorn};

use futures::future::{Future};
use futures::Stream;

use tokio_core::reactor::Handle;

use std::collections::HashMap;


#[derive(Deserialize, Debug)]
pub struct StateRecord {
    workers: i32,
    profile: String
}

pub type State = HashMap<String, StateRecord>;


pub fn read_from_unicron(path: &str) -> Option<State> {
    let mut core = Core::new().unwrap();
    let unicorn = Unicorn::new(Service::new("unicorn", &core.handle()));

    let future = unicorn.get::<State>(path);

    match core.run(future) {
        Ok((Some(data), version)) => {
            println!("version: {}, data: {:?}", version, data);
            Some(data)
        },
        Ok((None, _)) => {
            println!("no data");
            None
        },
        Err(error) => {
            println!("error: {:?}", error);
            None
        }
    }
}

pub fn read_from_storage(collection: &str, key: &str) -> Option<String> {
    let mut core = Core::new().unwrap();
    let storage = Storage::new(Service::new("storage", &core.handle()));

    println!("collection: {} key: {}", collection, key);

    let future = storage.read(collection, key);

    match core.run(future) {
        Ok(data) => {
            println!("data: {:?}", data);
            String::from_utf8(data).ok()
        }
        Err(error) => {
            println!("error: {:?}", error);
            None
        }
    }
}

pub fn unicorn_subscribe(service_name: &str, path: &str, handle: &Handle)
    // impl Future<Item=(), Error=Error>
    -> Box<Future<Item=(), Error=Error>>
{
    // TODO: secure token support
    let subsciption = Unicorn::new(Service::new(service_name.to_string(), handle))
        .subscribe::<State,_>(path, None);

    Box::new(
        subsciption.and_then(|(close, stream)| {
            stream.for_each(|(data, version)| {
                if let Some(data) = data {
                    println!("\t{}: {:?}", version, data);
                } else {
                    println!("\tno data in node");
                }
                Ok(())
            }).and_then(|_| {
                drop(close);
                Ok(())
            })
        })
    )
}
