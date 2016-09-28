use futures::stream::{self, Stream};
use futures::{self, Async, Future};
use tokio_service::Service;

use super::Empty;
use mpsc::{MpscSender, MpscReceiver};

use std::mem;

type HostMsg<S: Service> = (S::Request, futures::Complete<Result<S::Response, S::Error>>);

enum HostState<S: Service> {
    Idle,
    Blocked(S::Future, futures::Complete<Result<S::Response, S::Error>>),
}

pub struct Host<S: Service> {
    service: S,
    state: HostState<S>,

    rx: MpscReceiver<HostMsg<S>>,
}


impl<S: Service> Future for Host<S> {
    type Item = ();
    type Error = Empty;
    fn poll(&mut self) -> futures::Poll<(), Empty> {
        loop {
            if let HostState::Idle = self.state {
                match try_ready!(self.rx.poll()) {
                    Some((req, k)) => {
                        let resp = self.service.call(req);
                        self.state = HostState::Blocked(resp, k)
                    }
                    None => return Ok(Async::Ready(())),
                }
            };

            if let HostState::Blocked(mut f, k) = mem::replace(&mut self.state, HostState::Idle) {
                match f.poll() {
                    Ok(Async::Ready(r)) => {
                        k.complete(Ok(r));
                    }
                    Err(e) => {
                        k.complete(Err(e));
                    }
                    Ok(Async::NotReady) => {
                        self.state = HostState::Blocked(f, k);
                        return Ok(Async::NotReady);
                    }
                }
            }
        }
    }
}

pub struct Handle<S: Service> {
    tx: MpscSender<HostMsg<S>>,
}

impl<S: Service> Host<S> {
    pub fn build(s: S) -> (Handle<S>, Host<S>) {
        let (tx, rx) = MpscReceiver::pair();
        let handle = Handle { tx: tx.clone() };
        let host = Host {
            service: s,
            state: HostState::Idle,
            rx: rx,
        };
        (handle, host)
    }
}


impl<S: Service> Service for Handle<S> {
    type Request = S::Request;
    type Response = Result<S::Response, S::Error>;
    type Error = futures::Canceled;
    type Future = futures::Oneshot<Result<S::Response, S::Error>>;
    fn call(&self, req: S::Request) -> Self::Future {
        let (c, p) = futures::oneshot();
        let () = self.tx.send((req, c)).expect("send to tx");
        p
    }
    fn poll_ready(&self) -> Async<()> {
        Async::Ready(())
    }
}
