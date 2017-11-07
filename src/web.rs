//
// TODO: ugly & dirty fast coded implementation, rewrite/refactor someday.
// TODO: cache serialize strings, update on change?
//
use hyper::{
    self,
    Method,
    StatusCode
};

use std::sync::Arc;

use serde;
use serde_json;

use hyper::header::{ContentLength, ContentType};
use hyper::server::{Http, Request, Response};

use service_fn::service_fn;

use std::sync::RwLock;
use std::net::SocketAddr;

use std::collections::VecDeque;

use engine::SyncedCluster;
use orca::{
    SyncedOrcasPod,
    SyncedApps
};


static NOT_FOUND: &'static str = "<body><bold>Page not found</bold></body>";

pub struct Model {
    pub cluster: Arc<SyncedCluster>,
    pub orcas: Arc<SyncedOrcasPod>,
    pub apps: Arc<SyncedApps>,
}

#[derive(Debug)]
enum Route<'a> {
    Api(&'a str, &'a str),
    Asset(&'a str),
    Index
}

fn parse_path<'a>(path: &'a str) -> Route<'a> {
    let mut parts: VecDeque<_> = path.split('/').collect();
    parts.pop_front();

    println!("path parts {:?}", parts);

    match (parts.len(), parts.front()) {
        (0, _) => Route::Index,
        (3, Some(&"api")) => Route::Api(parts[1], parts[2]),
        _ => Route::Asset(path)
    }
}

fn mark_not_found(response: &mut Response, _path: &str) {
    response.set_status(StatusCode::NotFound);
    response.set_body(NOT_FOUND);
}

// TODO: remove arc from signature, should be pure reference.
fn set_json_body<T>(response: &mut Response, apps: Arc<RwLock<T>>)
where
    T: serde::de::DeserializeOwned + serde::ser::Serialize
{
    let apps = apps.read().unwrap();

    let body = serde_json::to_string(&*apps).unwrap();
    let len = body.len();

    response.set_body(body);
    response.headers_mut().set(ContentType::json());
    response.headers_mut().set(ContentLength(len as u64))
}

pub fn run(model: Arc<Model>) -> Result<(), hyper::Error> {
    let addr: SocketAddr = "[::1]:3000".parse().unwrap();

    let service = move || Ok(service_fn(|req: Request|{
        let mut response: Response = Response::new();

        let path = req.path();
        let command = parse_path(path);

        match (req.method(), command) {
            // TODO: static content.
            (&Method::Get, Route::Asset(_asset)) => response.set_body("asset"),
            // basic api implementation.
            (&Method::Get, Route::Api(ver, func)) => match (ver, func) {
                ("v1", "apps")    => set_json_body(&mut response, Arc::clone(&model.apps)),
                ("v1", "cluster") => set_json_body(&mut response, Arc::clone(&model.cluster)),
                ("v1", "orcas")   => set_json_body(&mut response, Arc::clone(&model.orcas)),
                _ => mark_not_found(&mut response, path)
            },
            _ => mark_not_found(&mut response, path)
        };

        Ok(response)
    }));

    let server = Http::new().bind(&addr, service)?;
    server.run()
}
