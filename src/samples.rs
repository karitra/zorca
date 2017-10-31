//
// TODO: for experiments, will be removed someday
//
use cocaine::{Core, Service};
use cocaine::service::{Storage, Unicorn};
use cocaine::hpack::RawHeader;

use futures::future::{Future};
use futures::sink::Sink;
use futures::sync::mpsc::{
    UnboundedSender,
};

use std::fmt::Debug;

use serde::Deserialize;
use std::collections::HashMap;

use errors::CombinedError;
use types::SubscribeMessage;


#[derive(Deserialize, Debug)]
pub struct StateRecord {
    workers: i32,
    profile: String
}

pub type State = HashMap<String, StateRecord>;

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
pub fn unicorn_kids_subscribe<H>(
    service: Service,
    path: &str,
    headers: H,
    sender: UnboundedSender<SubscribeMessage>)
    -> Box<Future<Item=(), Error=CombinedError>>
where
    H: Into<Option<Vec<RawHeader>>> + 'static,
{
    println!("subscribing to path {}", path);

    let subscription = Unicorn::new(service)
        .children_subscribe(path, headers)
        .map_err(CombinedError::CocaineError)
        .and_then(move |(tx, stream)| {
            println!("subscribed!");

            sender.sink_map_err(CombinedError::QueueSendError)
                .send_all(stream)
                .and_then(|_| {
                    drop(tx);
                    Ok(())
                })
        });

    Box::new(subscription)
}

#[allow(dead_code)]
pub fn unicorn_get_node<T>(service: Service, path: &str)
    -> Box<Future<Item=Option<T>, Error=CombinedError>>
where
    T: for<'de> Deserialize<'de> + Send + Debug + 'static,
{
    let future = Unicorn::new(service)
        .get::<T,_>(path, None)
        .and_then(|(maybe_data, _version)| {
            if let Some(data) = maybe_data {
                println!("content: {:?}", data);
                Ok(Some(data))
            } else {
                println!("content if node is empty");
                Ok(None)
            }
        }).map_err(CombinedError::CocaineError);

    Box::new(future)
}
