//! Multi-producer, multi-consumer sorted channel.
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use parking_lot::{Condvar, Mutex};

/// The sending-half of the [`sorted_channel`] type. This sender can be cloned
/// and sent to multiple threads.
///
/// # Examples
/// ```
/// use sorted_channel::sorted_channel;
/// use std::thread;
///
/// let (tx, rx) = sorted_channel();
/// let tx2 = tx.clone();
///
/// let h1 = thread::spawn(move || {
///     tx.send(2).unwrap();
///     tx.send(1).unwrap();
/// });
///
/// let h2 = thread::spawn(move || {
///     tx2.send(4).unwrap();
///     tx2.send(3).unwrap();
/// });
///
/// h1.join().unwrap();
/// h2.join().unwrap();
/// let outputs: Vec<_> = (0..4).map(|_| rx.recv().unwrap()).collect();
/// assert_eq!(outputs, [4, 3, 2, 1]);
/// ```
pub struct Sender<T> {
    inner: Arc<Inner<T>>,
}

/// An error returned from [`Sender::send`] when there are no [`Receiver`]s left
/// for the channel.
#[derive(Debug, PartialEq)]
pub struct SendError<T>(pub T);

/// The receiving-half of the [`sorted_channel`] type. This receiver can be
/// cloned and sent to multiple threads. See [`Sender`] for an example.
pub struct Receiver<T> {
    inner: Arc<Inner<T>>,
}

/// An error returned from [`Receiver::recv`] when there are no [`Sender`]s left
/// for the channel.
#[derive(Debug, PartialEq)]
pub struct RecvError;

struct Inner<T> {
    queue: Mutex<Vec<T>>,
    senders: AtomicUsize,
    receivers: AtomicUsize,
    new_values: Condvar,
}

impl<T: Ord> Sender<T> {
    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        if self.inner.receivers.load(Ordering::Relaxed) == 0 {
            return Err(SendError(t));
        }
        let mut queue = self.inner.queue.lock();
        queue.push(t);
        queue.sort_unstable();
        self.inner.new_values.notify_one();
        Ok(())
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        self.inner.senders.fetch_add(1, Ordering::Relaxed);
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.inner.senders.fetch_sub(1, Ordering::Relaxed);
    }
}

impl<T> Receiver<T> {
    /// Wait for data in the channel by blocking the current thread, returning
    /// an error if there are no more [`Sender`]s connected to this channel.
    ///
    /// # Examples
    /// ```
    /// use sorted_channel::sorted_channel;
    /// use std::thread;
    /// let (tx, rx) = sorted_channel();
    /// thread::spawn(move || {
    ///     tx.send(7).unwrap();
    /// });
    ///
    /// assert_eq!(rx.recv(), Ok(7));
    /// ```
    ///
    /// Failure from missing sender:
    /// ```
    /// use sorted_channel::sorted_channel;
    /// use std::thread;
    /// let (tx, rx) = sorted_channel::<()>();
    /// let handle = thread::spawn(move || {
    ///     drop(tx);
    /// });
    ///
    /// handle.join().unwrap();
    /// assert!(rx.recv().is_err());
    /// ```
    pub fn recv(&self) -> Result<T, RecvError> {
        let mut queue = self.inner.queue.lock();
        loop {
            match queue.pop() {
                Some(t) => return Ok(t),
                None => {
                    if self.inner.senders.load(Ordering::Relaxed) == 0 {
                        return Err(RecvError);
                    }
                    self.inner.new_values.wait(&mut queue);
                }
            }
        }
    }
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        self.inner.receivers.fetch_add(1, Ordering::Relaxed);
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.inner.receivers.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Creates a new sorted channel returning the [`Sender`]/[`Receiver`] halves.
///
/// # Examples
/// ```
/// use sorted_channel::sorted_channel;
/// use std::thread;
///
/// let (tx, rx) = sorted_channel();
///
/// let handle = thread::spawn(move || {
///     for i in [0, 9, 1, 8, 2, 7, 3, 6, 4, 5] {
///         tx.send(i).unwrap();
///     }
/// });
///
/// handle.join().unwrap();
///
/// for i in (0..10).rev() {
///     assert_eq!(rx.recv(), Ok(i));
/// }
/// ```
pub fn sorted_channel<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Inner {
        queue: Mutex::default(),
        senders: AtomicUsize::new(1),
        receivers: AtomicUsize::new(1),
        new_values: Condvar::new(),
    });

    (
        Sender {
            inner: inner.clone(),
        },
        Receiver { inner },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_receive_sorted() {
        let (tx, rx) = sorted_channel();

        for i in [0, 9, 1, 8, 2, 7, 3, 6, 4, 5] {
            tx.send(i).unwrap();
        }
        for i in (0..10).rev() {
            assert_eq!(rx.recv().unwrap(), i);
        }
    }

    #[test]
    fn no_senders() {
        let (tx, rx) = sorted_channel::<()>();
        drop(tx);
        assert_eq!(rx.recv(), Err(RecvError));
    }

    #[test]
    fn no_receivers() {
        let (tx, rx) = sorted_channel();
        drop(rx);
        let value = 7;
        assert_eq!(tx.send(7), Err(SendError(value)));
    }

    #[test]
    fn multiple_receivers() {
        let (tx, rx) = sorted_channel();
        let rx2 = rx.clone();
        for i in [0, 9, 1, 8, 2, 7, 3, 6, 4, 5] {
            tx.send(i).unwrap();
        }
        for i in (0..10).rev() {
            if i % 2 == 0 {
                assert_eq!(rx.recv().unwrap(), i);
            } else {
                assert_eq!(rx2.recv().unwrap(), i);
            }
        }
    }

    #[test]
    fn multiple_senders() {
        let (tx, rx) = sorted_channel();
        let tx2 = tx.clone();
        for i in [0, 9, 1, 8, 2, 7, 3, 6, 4, 5] {
            if i % 2 == 0 {
                tx.send(i).unwrap();
            } else {
                tx2.send(i).unwrap();
            }
        }
        for i in (0..10).rev() {
            assert_eq!(rx.recv().unwrap(), i);
        }
    }
}
