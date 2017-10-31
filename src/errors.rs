use cocaine;
use std;

use types::SubscribeMessage;
use futures::sync::mpsc::SendError;


#[derive(Debug)]
pub enum CombinedError {
    QueueSendError(SendError<SubscribeMessage>),
    CocaineError(cocaine::Error),
    IOError(std::io::Error),
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
