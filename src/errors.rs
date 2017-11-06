use cocaine;
use std;
use hyper;
use serde_json;

use futures::sync::mpsc::SendError;

use engine::SubscribeMessage;

#[derive(Debug)]
pub enum CombinedError {
    UriParseError(hyper::error::UriError),
    QueueSendError(SendError<SubscribeMessage>),
    CocaineError(cocaine::Error),
    IOError(std::io::Error),
    HyperError(hyper::Error),
    SerdeError(serde_json::Error),
    Other(String),
}

impl From<SendError<SubscribeMessage>> for CombinedError {
    fn from(err: SendError<SubscribeMessage>) -> Self {
        CombinedError::QueueSendError(err)
    }
}

impl From<cocaine::Error> for CombinedError {
    fn from(err: cocaine::Error) -> Self {
        CombinedError::CocaineError(err)
    }
}

impl From<std::io::Error> for CombinedError {
    fn from(err: std::io::Error) -> Self {
        CombinedError::IOError(err)
    }
}

impl From<hyper::Error> for CombinedError {
    fn from(err: hyper::Error) -> Self {
        CombinedError::HyperError(err)
    }
}
