use crate::Result;
use alloy::api::{APIRequest, APIResult, Message, SetRequest, SubscriptionRequest};
use alloy::config::VirtualDeviceConfig;
use alloy::event::AddressedEvent;
use alloy::tcp::Connection;
use alloy::{Address, Value};
use failure::err_msg;
use std::sync::Arc;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::{oneshot, Mutex};
use tokio::task;

pub(crate) struct Client {
    messages_out: Sender<Message>,
    inner: Arc<Mutex<ClientInner>>,
    event_stream: Option<Receiver<Vec<AddressedEvent>>>,
}

struct ClientInner {
    results: Vec<Option<oneshot::Sender<Result<APIResult>>>>,
    current_id: u16,
}

impl Client {
    pub(crate) async fn new<A: ToSocketAddrs>(addr: A) -> Result<Client> {
        let conn = TcpStream::connect(addr).await?;
        conn.set_nodelay(true)?;

        Self::new_from_conn(conn).await
    }

    pub(crate) async fn new_from_conn(conn: TcpStream) -> Result<Client> {
        let conn = Connection::new(conn).await?;

        let messages_out = conn.messages_out.clone();
        let messages_in = conn.messages_in;
        let (events_tx, events_rx) = channel::<Vec<AddressedEvent>>(100);

        let mut ci = ClientInner {
            results: Vec::with_capacity(4096),
            current_id: 0,
        };
        for _i in 0..4096 {
            ci.results.push(None);
        }

        let inner = Arc::new(Mutex::new(ci));
        let inner2 = inner.clone();

        task::spawn(Self::handle_incoming_messages(
            inner2,
            messages_in,
            events_tx,
        ));

        Ok(Client {
            messages_out,
            inner,
            event_stream: Some(events_rx),
        })
    }

    async fn handle_incoming_messages(
        inner: Arc<Mutex<ClientInner>>,
        mut messages_in: Receiver<Message>,
        mut events_out: Sender<Vec<AddressedEvent>>,
    ) {
        while let Some(msg) = messages_in.recv().await {
            match msg {
                Message::Version(_) => {
                    error!("received unexpected version message");
                    break;
                }
                Message::Events(events) => {
                    let res = events_out.send(events).await;
                    match res {
                        Err(e) => {
                            error!("unable to pass on events: {:?}", e);
                            break;
                        }
                        Ok(()) => {}
                    }
                }
                Message::Request { id: _, inner: _ } => {
                    error!("received unexpected request message");
                    break;
                }
                Message::Response { id, inner: res } => {
                    debug!("got response {} => {:?}", id, res);
                    let mut inner = inner.lock().await;
                    let receiver = inner.results[id as usize].take();
                    if receiver.is_none() {
                        error!("received response without request? {} => {:?}", id, res);
                        break;
                    }

                    receiver.unwrap().send(res.map_err(|e| err_msg(e))).unwrap();
                }
            }
        }

        debug!("handler shutting down")
    }

    async fn perform_request(&self, req: APIRequest) -> Result<APIResult> {
        let (id, receiver) = {
            let mut inner = self.inner.lock().await;
            let next_id = (inner.current_id + 1) % 4096;
            if inner.results[next_id as usize].is_some() {
                // TODO wait
                return Err(err_msg("too many open requests!"));
            }

            inner.current_id = next_id;
            let (tx, rx) = oneshot::channel();
            inner.results[next_id as usize] = Some(tx);
            (next_id, rx)
        };

        self.messages_out
            .clone()
            .send(Message::Request { id, inner: req })
            .await?;

        receiver.await.unwrap()
    }

    pub(crate) fn event_stream(&mut self) -> Result<Receiver<Vec<AddressedEvent>>> {
        let s = self.event_stream.take();
        match s {
            None => Err(err_msg("event stream already taken")),
            Some(s) => Ok(s),
        }
    }

    pub(crate) async fn ping(&self) -> Result<()> {
        let res = self.perform_request(APIRequest::Ping).await?;
        match res {
            APIResult::Ping => Ok(()),
            _ => {
                // what do?
                panic!(format!(
                    "received invalid response, expected Ping, got {:?}",
                    res
                ))
            }
        }
    }

    pub(crate) async fn devices(&self) -> Result<Vec<VirtualDeviceConfig>> {
        let res = self.perform_request(APIRequest::Devices).await?;
        match res {
            APIResult::Devices(devices) => Ok(devices),
            _ => {
                // what do?
                panic!(format!(
                    "received invalid response, expected Devices, got {:?}",
                    res
                ))
            }
        }
    }

    pub(crate) async fn set(&self, req: Vec<SetRequest>) -> Result<()> {
        let res = self.perform_request(APIRequest::Set(req)).await?;
        match res {
            APIResult::Set => Ok(()),
            _ => {
                // what do?
                panic!(format!(
                    "received invalid response, expected Set, got {:?}",
                    res
                ))
            }
        }
    }

    pub(crate) async fn get(&self, addr: Address) -> Result<Value> {
        let res = self.perform_request(APIRequest::Get(addr)).await?;
        match res {
            APIResult::Get(v) => Ok(v),
            _ => {
                // what do?
                panic!(format!(
                    "received invalid response, expected Get, got {:?}",
                    res
                ))
            }
        }
    }

    pub(crate) async fn subscribe(&self, req: SubscriptionRequest) -> Result<()> {
        let res = self.perform_request(APIRequest::Subscribe(req)).await?;
        match res {
            APIResult::Subscribe => Ok(()),
            _ => {
                // what do?
                panic!(format!(
                    "received invalid response, expected Subscribe, got {:?}",
                    res
                ))
            }
        }
    }
}
