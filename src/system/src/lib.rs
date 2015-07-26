
extern crate lease;
extern crate shared_future;
extern crate snowstorm;
extern crate entity;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub mod channel {
    pub use snowstorm::channel::*;
}

pub struct System<Message:Send+Sync, Data:Send+Sync> {
    front: lease::Owner<Data>,
    back: lease::Owner<Data>,
    set: shared_future::Set<SystemHandle<Message, Data>>,
    count: Arc<AtomicUsize>,
    input: channel::Receiver<Message>
}

pub struct SystemHandle<Message:Send+Sync, Data:Send+Sync>{
    data: lease::Lease<Data>,
    channel: channel::Sender<Message>,
    count: Arc<AtomicUsize>,
    next: shared_future::Future<SystemHandle<Message, Data>>
}

impl<M, D> Clone for SystemHandle<M, D>
    where M: Send + Sync,
          D: Send + Sync
{
    fn clone(&self) -> SystemHandle<M, D> {
        SystemHandle{
            data: self.data.clone(),
            channel: self.channel.clone(),
            next: self.next.clone(),
            count: self.count.clone()
        }
    }
}

impl<M, D> SystemHandle<M, D> 
    where M: Send + Sync,
          D: Send + Sync
{

    /// Flush all changes and try and fetch the next update for this system
    /// Returns true of the system was updated, false if it was not
    pub fn next_frame(self) -> shared_future::Future<SystemHandle<M, D>> {
        let SystemHandle{data, channel, next, count} = self;
        count.fetch_add(1, Ordering::SeqCst);
        drop((data, channel));
        next
    }

    /// Sends a message with to the system via the included channel
    /// The channels are buffered and therefore the delivery is not
    /// guaranteed to occur immediately
    pub fn send(&mut self, m: M) {
        self.channel.send(m);
    }
}

impl<M, D> std::ops::Deref for SystemHandle<M, D>
    where M: Send + Sync,
          D: Send + Sync
{
    type Target = D;

    fn deref(&self) -> &D { &self.data }
}

impl<M, D> System<M, D>
    where M: Send + Sync,
          D: Send + Sync
{
    /// Create a new system and a handle to it. That data is cloned
    /// into the front and back buffer of the channel
    pub fn new(front: D, back: D) -> (System<M, D>, SystemHandle<M, D>) {
        let (front, l) = lease::lease(front);
        let (back, _) = lease::lease(back);
        let (future, set) = shared_future::Future::new();
        let (sender, input) = channel::channel();

        let count = Arc::new(AtomicUsize::new(0));

        let system = System{
            front: front,
            back: back,
            set: set,
            input: input,
            count: count.clone()
        };

        let handle = SystemHandle{
            data: l,
            channel: sender,
            next: future,
            count: count
        };

        (system, handle)
    }

    /// Update the system
    pub fn update<F>(self, f: F) -> Option<System<M, D>>
        where F: FnOnce(D, &D, channel::Receiver<M>) -> D
    {
        let System{front, back, set, input, count} = self;
        let (next, l) = lease::lease(f(back.get(), &*front, input));

        if count.load(Ordering::SeqCst) == 0 {
            return None;
        }

        let (sender, input) = channel::channel();
        let (future, nset) = shared_future::Future::new();
        let count = Arc::new(AtomicUsize::new(0));

        // Show the updated state to the outside world
        set.set(SystemHandle{
            data: l,
            channel: sender,
            next: future,
            count: count.clone()
        });

        Some(System{
            front: next,
            back: front,
            set: nset,
            input: input,
            count: count
        })
    }
}

impl<T, U, V> entity::DeleteEntity<T> for SystemHandle<entity::Operation<T, U>, V> 
    where T: Send+Sync,
          U: Send+Sync,
          V: Send+Sync
{
    fn delete(&mut self, eid: T) {
        self.send(entity::Operation::Delete(eid));
    }
}