use cocaine::{Core, Error, Service};
use cocaine::service::{Storage, Unicorn, Tvm};
use cocaine::service::tvm::Grant;

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

pub fn get_ticket(service_name: &str, id: u32, secret: &str, grant: &Grant, handle: &Handle)
    -> Box<Future<Item=String, Error=Error>>
{
    Box::new(
        Tvm::new(Service::new(service_name.to_string(), handle))
        .ticket(id, secret, grant)
    )
}

pub fn unicorn_kids_subscribe(service_name: &str, path: &str, handle: &Handle)
    -> Box<Future<Item=(), Error=Error>>
{
    let subscription = Unicorn::new(Service::new(service_name.to_string(), handle))
        .children_subscribe(path);

    Box::new(
        subscription.and_then(|(tx, stream)| {
            println!("subscribed");
            stream.for_each(|(version, nodes)| {
                for node in nodes {
                    println!("{}: {:?}", version, node);
                }
                Ok(())
            }).and_then(|_| {
                drop(tx);
                Ok(())
            })
        })
    )
}

pub fn unicorn_get_node(service_name: &str, path: &str, handle: &Handle)
    -> Box<Future<Item=Option<String>, Error=Error>>
{
    let future = Unicorn::new(Service::new(service_name.to_string(), handle))
        .get::<String>(path);

    Box::new(
        future.and_then(|(maybe_data, _version)| {
            if let Some(data) = maybe_data {
                println!("content: {}", data);
                Ok(Some(data))
            } else {
                println!("content if node is empty");
                Ok(None)
            }
        })
    )
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
