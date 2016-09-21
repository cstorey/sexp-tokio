use futures::stream::{self, Stream};
use futures::{self, Async};
use std::sync::mpsc;

use super::Empty;

pub struct MpscReceiver<T> {
    rx: mpsc::Receiver<T>,
}

impl<T> MpscReceiver<T> {
    pub fn pair() -> (mpsc::Sender<T>, Self) {
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
