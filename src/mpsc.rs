use futures::stream::{self, Stream};
use futures::{self, Async};
use std::sync::mpsc;

use super::Empty;

pub struct MpscReceiver<T> {
    rx: mpsc::Receiver<T>,
}

#[derive(Debug)]
pub struct MpscSender<T> {
    tx: mpsc::Sender<T>,
}

impl<T> Clone for MpscSender<T> {
    fn clone(&self) -> Self {
        MpscSender { tx: self.tx.clone() }
    }
}

impl<T> MpscSender<T> {
    pub fn send(&self, val: T) -> Result<(), mpsc::SendError<T>> {
        self.tx.send(val)
    }
}

impl<T> MpscReceiver<T> {
    pub fn pair() -> (MpscSender<T>, Self) {
        let (tx, rx) = mpsc::channel();
        let recv = MpscReceiver { rx: rx };
        let send = MpscSender { tx: tx };
        (send, recv)
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
