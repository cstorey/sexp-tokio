extern crate log;
extern crate env_logger;
extern crate tokio_service;
#[macro_use]
extern crate futures;
use futures::{Future, IntoFuture, Async};
use futures::task;
use futures::task::{Unpark, Spawn};
use futures::stream::{self, Stream};

use tokio_service::Service;
use std::sync::Arc;
use std::sync::mpsc;
use std::mem;
use std::fmt;
use std::collections::VecDeque;

#[derive(Debug)]
struct Unparker;

impl Unpark for Unparker {
    fn unpark(&self) {
        println!("Unpark! {:?}", self);

    }
}

#[derive(Debug)]
enum Empty {}
struct MyService;
unsafe impl Send for MyService {}

impl Service for MyService {
    type Request = u64;
    type Response = u64;
    type Error = Empty;
    type Future = futures::Finished<Self::Response, Self::Error>;
    fn call(&self, req: u64) -> Self::Future {
        println!("MyService#call: {:?}", req);
        futures::finished(req + 1)
    }
    fn poll_ready(&self) -> Async<()> {
        Async::Ready(())
    }
}

struct MpscReceiver<T> {
    rx: mpsc::Receiver<T>,
}

impl<T> MpscReceiver<T> {
    fn pair() -> (mpsc::Sender<T>, Self) {
        let (tx, rx) = mpsc::channel();
        let recv = MpscReceiver { rx: rx };
        (tx, recv)
    }
}

impl<T> Stream for MpscReceiver<T> {
    type Item = T;
    type Error = Empty;

    fn poll(&mut self) -> futures::Poll<Option<T>, Empty> {
        match self.rx.try_recv() {
            Ok(val) => Ok(Async::Ready(Some(val))),
            Err(mpsc::TryRecvError::Empty) => return Ok(Async::NotReady),
            Err(mpsc::TryRecvError::Disconnected) => return Ok(Async::Ready(None)),
        }
    }
}


type HostMsg<S: Service> = (S::Request, futures::Complete<Result<S::Response, S::Error>>);

enum HostState<S: Service> {
    Idle,
    Blocked(S::Future, futures::Complete<Result<S::Response, S::Error>>),
}

struct Host<S: Service> {
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

struct Handle<S: Service> {
    tx: mpsc::Sender<HostMsg<S>>,
}

impl<S: Service> Host<S> {
    fn build(s: S) -> (Handle<S>, Host<S>) {
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


impl<S: Service> Service for Handle<S>
    where S::Request: fmt::Debug
{
    type Request = S::Request;
    type Response = Result<S::Response, S::Error>;
    type Error = futures::Canceled;
    type Future = futures::Oneshot<Result<S::Response, S::Error>>;
    fn call(&self, req: S::Request) -> Self::Future {
        let (c, p) = futures::oneshot();
        println!("Handle#call: {:?}", req);
        let () = self.tx.send((req, c)).expect("send to tx");
        p
    }
    fn poll_ready(&self) -> Async<()> {
        Async::Ready(())
    }
}

fn main() {
    type Resp = Result<u64, Empty>;
    type Reqs = (u64, futures::Complete<Resp>);

    let (client, srv) = Host::build(MyService);

    let unparker = Arc::new(Unparker);

    println!("Setup done!");
    let mut tasks: VecDeque<Spawn<Box<Future<Item = (), Error = Empty>>>> = VecDeque::new();

    let f = futures::lazy(move || {
        (client.call(32), client.call(42), client.call(89))
            .into_future()
            .map_err(|e| panic!("{:?}", e))
            .map(|r| println!("Result: {:?}", r))
    });

    tasks.push_back(task::spawn(f.boxed()));
    tasks.push_back(task::spawn(srv.boxed()));

    let unparker = Arc::new(Unparker);
    while let Some(mut t) = tasks.pop_front() {
        println!("Pre poll");
        match t.poll_future(unparker.clone()) {
            Ok(Async::Ready(v)) => {
                println!("done: {:?}", v);
            }
            Ok(Async::NotReady) => {
                println!("pending:");
                tasks.push_back(t);
            }
            Err(e) => println!("Error: {:?}", e),
        }
    }
}
