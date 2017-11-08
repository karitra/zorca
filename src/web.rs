//
// TODO: ugly & dirty fast coded implementation, rewrite/refactor someday.
// TODO: cache serialized strings, update on change?
//
use futures::{Future, future};
use tokio_core::reactor::Handle;

use serde;
use serde_json;

use hyper_staticfile::Static;

use hyper::header::{ContentLength, ContentType};
use hyper::server::{Request, Response, Service};
use hyper::{
    Error,
    Method,
    StatusCode
};

use std::sync::{Arc, RwLock};
use std::path::Path;
use std::ops::Deref;
use std::collections::VecDeque;

use engine::SyncedCluster;
use orca::{
    SyncedOrcasPod,
    SyncedApps
};


const API_V1: &str = "v1";
static NOT_FOUND: &str = "<html>\
    <body>\
    <h1>Page not found</h1>\
    </body>\
    </html>";


type BoxedResponseFuture = Box<Future<Item=Response, Error=Error>>;

#[derive(Clone)]
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

fn parse_path(path: &str) -> Route {
    let mut parts: VecDeque<_> = path.split('/').collect();
    parts.pop_front(); // normal path always contains '/' at beginning

    // println!("path parts {:?}", parts);

    match (parts.len(), parts.front()) {
        (0, _) => Route::Index,
        (3, Some(&"api")) => Route::Api(parts[1], parts[2]),
        _ => Route::Asset(path)
    }
}

fn as_not_found(_path: &str)
    -> BoxedResponseFuture
{
    let mut response = Response::new();

    response.set_status(StatusCode::NotFound);
    response.set_body(NOT_FOUND);

    Box::new(future::ok(response))
}

fn as_json_locked<T>(item: Arc<RwLock<T>>)
    -> BoxedResponseFuture
where
    T: serde::ser::Serialize
{
    let mut response = Response::new();

    let unlocked = item.read().unwrap();
    set_json_body(&mut response, unlocked.deref());

    Box::new(future::ok(response))
}

fn set_json_body<T>(response: &mut Response, item: &T)
where
    T: serde::ser::Serialize
{
    let body = match serde_json::to_string(item) {
        Ok(b) => b,
        Err(_) => r#"{"error": "internal error"}"#.into()
    };

    let len = body.len();

    response.set_body(body);
    response.headers_mut().set(ContentType::json());
    response.headers_mut().set(ContentLength(len as u64))
}

pub struct WebApi {
    model: Model,
    static_content: Static,
}

impl WebApi {
    pub fn new(handle: &Handle, model: Model, static_path: &str) -> WebApi {
        WebApi {
            model,
            static_content: Static::new(handle, Path::new(static_path))
        }
    }
}

impl Service for WebApi {
    type Response = Response;
    type Request = Request;
    type Error = Error;
    type Future = BoxedResponseFuture;

    fn call(&self, request: Request) -> Self::Future {
        let path = request.path().to_string();
        let command = parse_path(&path);

        let response = match (request.method(), command) {
            // Serve static content.
            (&Method::Get, Route::Asset(_asset)) => {
                println!("asset is {:?}", _asset);
                self.static_content.call(request)
            },
            // Basic api implementation.
            (&Method::Get, Route::Api(ver, func)) => match (ver, func) {
                (API_V1, "apps")    => as_json_locked(Arc::clone(&self.model.apps)),
                (API_V1, "cluster") => as_json_locked(Arc::clone(&self.model.cluster)),
                (API_V1, "orcas")   => as_json_locked(Arc::clone(&self.model.orcas)),
                (API_V1, "self")    => as_not_found(request.path()), // TODO: self info: version etc
                _ => as_not_found(&path)
            },
            _ => as_not_found(&path)
        };

        Box::new(response)
    }
}
