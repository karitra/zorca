//
// TODO: for experiments, will be removed someday
//
use cocaine::{Core, Error, Service};
use cocaine::service::{Storage, Unicorn};
use cocaine::hpack::RawHeader;

use futures;
use futures::future::{Future};
use futures::{StartSend, Stream};
use futures::sink::Sink;

use futures::sync::mpsc::UnboundedSender;

use tokio_core::reactor::Handle;

use std::fmt::Debug;

use serde::Deserialize;
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

    let future = unicorn.get::<State,_>(path, None);

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

pub fn unicorn_kids_subscribe<H>(service: Service, path: &str, headers: H, handle: Handle, mut sender: UnboundedSender<(String,Vec<String>)>)
    -> Box<Future<Item=(), Error=Error>>
where
    H: Into<Option<Vec<RawHeader>>> + 'static,
{
    println!("subscribing...");

    let path_string = path.to_string();

    let subscription = Unicorn::new(service)
        .children_subscribe(path, headers)
        .and_then(move |(tx, stream)| {
            println!("subscribed!");
            // TODO: bind stream directly to queue, is it possible?
            stream.for_each(move |(version, nodes)| {
                println!("version: {}, items {}", version, nodes.len());

                for node in &nodes {
                    println!("\t{:?}", node);
                }

                if let Err(e) = sender.start_send((path_string.clone(), nodes)) {
                    eprintln!("failed to put to queue {:?}", e);
                }

                Ok(())
            }).and_then(|_| {
                drop(tx);
                Ok(())
            })
        });

    Box::new(subscription)
}

pub fn unicorn_get_node<T>(service_name: &str, path: &str, handle: &Handle)
    -> Box<Future<Item=Option<T>, Error=Error>>
where
    T: for<'de> Deserialize<'de> + Send + Debug + 'static,
{
    let future = Unicorn::new(Service::new(service_name.to_string(), handle))
        .get::<T,_>(path, None)
        .and_then(|(maybe_data, _version)| {
            if let Some(data) = maybe_data {
                println!("content: {:?}", data);
                Ok(Some(data))
            } else {
                println!("content if node is empty");
                Ok(None)
            }
        });

    Box::new(future)
}

pub fn unicorn_subscribe<T>(service_name: &str, path: &str, handle: &Handle)
    -> Box<Future<Item=(), Error=Error>>
    // -> impl Future<Item=(), Error=Error>
where
    T: for<'de> Deserialize<'de> + Send + Debug + 'static,
{
    let subsciption = Unicorn::new(Service::new(service_name.to_string(), handle))
        .subscribe::<State,_>(path, None)
        .and_then(|(close, stream)| {
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
            });

    Box::new(subsciption)
}
