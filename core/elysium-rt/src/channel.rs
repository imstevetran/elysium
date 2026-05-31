// Channel implementation for Elysium concurrency
// Safe message passing between tasks.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

/// A bounded or unbounded channel for communicating between tasks.
pub struct Channel<T> {
    inner: Arc<Mutex<ChannelInner<T>>>,
    sender: Sender<()>,
    receiver: Arc<Mutex<std::sync::mpsc::Receiver<()>>>,
}

struct ChannelInner<T> {
    buffer: VecDeque<T>,
    capacity: Option<usize>,
    closed: bool,
}

impl<T: Send + 'static> Channel<T> {
    pub fn new(capacity: Option<usize>) -> Self {
        let (tx, rx) = mpsc::channel();
        Channel {
            inner: Arc::new(Mutex::new(ChannelInner {
                buffer: VecDeque::new(),
                capacity,
                closed: false,
            })),
            sender: tx,
            receiver: Arc::new(Mutex::new(rx)),
        }
    }

    pub fn send(&self, value: T) -> Result<(), &'static str> {
        let mut inner = self.inner.lock().unwrap();
        if inner.closed {
            return Err("channel is closed");
        }
        if let Some(cap) = inner.capacity {
            while inner.buffer.len() >= cap {
                drop(inner);
                let rx = self.receiver.lock().unwrap();
                rx.recv().ok();
                inner = self.inner.lock().unwrap();
            }
        }
        inner.buffer.push_back(value);
        let _ = self.sender.send(());
        Ok(())
    }

    pub fn receive(&self) -> Result<T, &'static str> {
        let mut inner = self.inner.lock().unwrap();
        while inner.buffer.is_empty() && !inner.closed {
            drop(inner);
            let rx = self.receiver.lock().unwrap();
            rx.recv().map_err(|_| "channel receive error")?;
            inner = self.inner.lock().unwrap();
        }
        inner.buffer.pop_front().ok_or("channel is empty and closed")
    }

    pub fn close(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.closed = true;
        let _ = self.sender.send(());
    }
}

impl<T> Clone for Channel<T> {
    fn clone(&self) -> Self {
        Channel {
            inner: Arc::clone(&self.inner),
            sender: self.sender.clone(),
            receiver: Arc::clone(&self.receiver),
        }
    }
}
