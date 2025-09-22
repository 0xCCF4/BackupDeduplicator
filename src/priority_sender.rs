use event_listener::{Event, Listener};
use std::collections::BinaryHeap;
use std::sync::mpsc::{RecvError, SendError, TryRecvError};
use std::sync::{Arc, Mutex};

pub fn channel<T: Ord>() -> (PrioritySender<T>, PriorityReceiver<T>) {
    let channel = Arc::new(PriorityChannel {
        queue: Mutex::new(BinaryHeap::new()),
        event_new_item: Event::new(),
        state: Mutex::new(ChannelState {
            closed: false,
            number_of_items: 0,
            sender_count: 1,
        }),
    });
    let sender = PrioritySender {
        inner: Arc::clone(&channel),
    };
    let receiver = PriorityReceiver { inner: channel };
    (sender, receiver)
}

struct ChannelState {
    closed: bool,
    number_of_items: usize,
    sender_count: usize,
}

struct PriorityChannel<T: Ord> {
    queue: Mutex<BinaryHeap<T>>,
    state: Mutex<ChannelState>,

    event_new_item: Event,
}

pub struct PrioritySender<T: Ord> {
    inner: Arc<PriorityChannel<T>>,
}

impl<T: Ord> Clone for PrioritySender<T> {
    fn clone(&self) -> Self {
        let mut state = self
            .inner
            .state
            .lock()
            .expect("priority sender lock failed");

        if state.sender_count == usize::MAX {
            panic!("too many senders - integer overflow")
        }

        state.sender_count += 1;

        let inner = Arc::clone(&self.inner);
        PrioritySender { inner }
    }
}

impl<T: Ord> Drop for PrioritySender<T> {
    fn drop(&mut self) {
        drop(self.inner.state.lock().map(|mut state| {
            let new_count = state.sender_count.saturating_sub(1);
            state.sender_count = new_count;
            if new_count == 0 {
                state.closed = true;
                self.inner.event_new_item.notify(usize::MAX);
            }
        }))
    }
}

unsafe impl<T: Send + Ord> Send for PrioritySender<T> {}
unsafe impl<T: Send + Ord> Sync for PrioritySender<T> {}

pub struct PriorityReceiver<T: Ord> {
    inner: Arc<PriorityChannel<T>>,
}

impl<T: Ord> PrioritySender<T> {
    pub fn send(&self, item: T) -> Result<(), SendError<T>> {
        let mut state = match self.inner.state.lock() {
            Ok(guard) => guard,
            Err(_poisoned) => return Err(SendError(item)),
        };
        let mut queue = match self.inner.queue.lock() {
            Ok(guard) => guard,
            Err(_poisoned) => return Err(SendError(item)),
        };

        queue.push(item);
        state.number_of_items += 1;

        drop(state);
        drop(queue);

        self.inner.event_new_item.notify(1);

        Ok(())
    }
}

impl<T: Ord> PriorityReceiver<T> {
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        let mut state = match self.inner.state.lock() {
            Ok(guard) => guard,
            Err(_poisoned) => return Err(TryRecvError::Disconnected),
        };
        if state.number_of_items > 0 {
            match self.inner.queue.lock() {
                Ok(mut queue) => {
                    let item = queue.pop().expect("number_of_items > 0 but queue is empty");
                    state.number_of_items -= 1;
                    Ok(item)
                }
                Err(_poisoned) => Err(TryRecvError::Disconnected),
            }
        } else if state.closed {
            Err(TryRecvError::Disconnected)
        } else {
            Err(TryRecvError::Empty)
        }
    }

    pub fn recv(&self) -> Result<T, RecvError> {
        loop {
            match self.try_recv() {
                Ok(item) => return Ok(item),
                Err(TryRecvError::Disconnected) => return Err(RecvError),
                Err(TryRecvError::Empty) => self.inner.event_new_item.listen().wait(),
            }
        }
    }
}
