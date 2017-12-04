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
use std::time::{self, UNIX_EPOCH};

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

    pub self_info: SelfInfo,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SelfInfo {
    start_time: u64,
    uptime: u64,
    version: String,
}

impl SelfInfo {
    pub fn new(version: &str) -> SelfInfo {
        let start_time = time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let start_time = start_time.as_secs();
        let version = version.to_string();

        SelfInfo {start_time, version, uptime: 0}
    }

    pub fn as_json_response(&self) -> BoxedResponseFuture
    {
        let mut response = Response::new();
        let now = time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let now = now.as_secs();

        let to_display = SelfInfo { uptime: now - self.start_time, ..self.clone() };
        set_json_body(&mut response, &to_display);

        Box::new(future::ok(response))
    }

}

#[derive(Debug)]
enum Route<'a> {
    Api(&'a str, &'a str),
    Asset(&'a str),
}

fn parse_path(path: &str) -> Route {
    // println!("path to parse {:?}", path);

    let mut parts: VecDeque<_> = path.split('/').collect();
    parts.pop_front(); // normal path always contains '/' at beginning

    // println!("path parts {:?}", parts);

    match (parts.len(), parts.front()) {
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

fn as_json_locked<T>(item: &RwLock<T>)
    -> BoxedResponseFuture
where
    T: serde::ser::Serialize
{
    let mut response = Response::new();

    let unlocked = item.read().unwrap();
    set_json_body(&mut response, unlocked.deref());

    Box::new(future::ok(response))
}

#[allow(dead_code)]
fn as_json<T>(item: Arc<T>) -> BoxedResponseFuture
where
    T: serde::ser::Serialize
{
    let mut response = Response::new();
    set_json_body(&mut response, item.as_ref());
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
                (API_V1, "apps")    => as_json_locked(self.model.apps.as_ref()),
                (API_V1, "cluster") => as_json_locked(self.model.cluster.as_ref()),
                (API_V1, "orcas") | (API_V1, "pod")
                                    => as_json_locked(self.model.orcas.as_ref()),
                (API_V1, "self")    => self.model.self_info.as_json_response(), // TODO: self info: version etc
                _ => as_not_found(&path)
            },

            _ => as_not_found(&path)
        };

        Box::new(response)
    }
}
