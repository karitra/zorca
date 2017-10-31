use cocaine::Service;
use cocaine::hpack::RawHeader;
use cocaine::service::Unicorn;

use futures::Future;
use futures::sync::mpsc::SendError;
use futures::sink::Sink;

use errors::CombinedError;
use types::SubscribeMessage;


pub fn kids_subscribe<'a, H, Q>(service: Service, path: &str, headers: H, sender: Q)
    -> Box<Future<Item=(), Error=CombinedError> + 'a>
where
    H: Into<Option<Vec<RawHeader>>> + 'a,
    Q: Sink<SinkItem=SubscribeMessage, SinkError=SendError<SubscribeMessage>> + 'a
{
    let subscription =
        Unicorn::new(service)
            .children_subscribe(path, headers)
            .map_err(CombinedError::CocaineError)
            .and_then(move |(tx, stream)| {
                sender.sink_map_err(CombinedError::QueueSendError)
                    .send_all(stream)
                    .and_then(|_| {
                        drop(tx);
                        Ok(())
                    })
            });

    Box::new(subscription)
}
