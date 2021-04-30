#![warn(missing_docs)]

//! Single-threaded polling-based executor suitable for use in games, embedded systems or WASM.
//!
//! This crate provides an executor to run async functions in  single-threaded
//! environment with deterministic execution. To do so, the executor provides a [`Executor::step()`]
//! method that polls all non-blocked tasks exactly once. The executor also provides events
//! that can be waited upon. These events are referred to by the [`EventHandle`] type which is
//! instantiated by calling [`Executor::create_event_handle()`], and can be waited on by
//! creating [`EventFuture`] by calling the [`Executor::event()`] method. They can be activated
//! by calling the [`Executor::notify_event()`] method.
//!
//! # Example
//! ```
//! # use simple_async_local_executor::*;
//! let executor = Executor::default();
//! let events = [executor.create_event_handle(), executor.create_event_handle()];
//!
//! async fn wait_event(events: [EventHandle; 2], executor: Executor) {
//!     executor.event(&events[0]).await;
//!     executor.event(&events[1]).await;
//! }
//!
//! executor.spawn(wait_event(events.clone(), executor.clone()));
//! assert_eq!(executor.step(), true);
//! assert_eq!(executor.step(), true);
//! executor.notify_event(&events[0]);
//! assert_eq!(executor.step(), true);
//! executor.notify_event(&events[1]);
//! assert_eq!(executor.step(), false);
//! ```

use core::fmt;
use std::{
    cell::{Cell, RefCell},
    future::Future,
    hash::{Hash, Hasher},
    pin::Pin,
    ptr,
    rc::Rc,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

#[cfg(feature = "futures")]
use futures::future::FusedFuture;
use slab::Slab;

// Useful reading for people interested in writing executors:
// - https://os.phil-opp.com/async-await/
// - https://rust-lang.github.io/async-book/02_execution/01_chapter.html
// - https://github.com/rust-lang/rfcs/blob/master/text/2592-futures.md#rationale-drawbacks-and-alternatives-to-the-wakeup-design-waker

fn dummy_raw_waker() -> RawWaker {
    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        dummy_raw_waker()
    }

    let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);
    RawWaker::new(std::ptr::null::<()>(), vtable)
}
fn dummy_waker() -> Waker {
    unsafe { Waker::from_raw(dummy_raw_waker()) }
}

#[derive(Clone)]
struct EventHandleInner {
    index: usize,
    executor: Rc<ExecutorInner>,
}
impl fmt::Debug for EventHandleInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.index.fmt(f)
    }
}
impl Eq for EventHandleInner {}
impl PartialEq for EventHandleInner {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && ptr::eq(self.executor.as_ref(), other.executor.as_ref())
    }
}
impl Hash for EventHandleInner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        (self.executor.as_ref() as *const ExecutorInner).hash(state);
    }
}
impl Drop for EventHandleInner {
    fn drop(&mut self) {
        self.executor.release_event_handle(self);
    }
}
/// A handle for an event, can be kept and cloned around
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EventHandle(Rc<EventHandleInner>);

type SharedBool = Rc<Cell<bool>>;

/// A future to await an event
pub struct EventFuture {
    ready: SharedBool,
    _handle: EventHandle,
    done: bool,
}
impl Future for EventFuture {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
        if self.ready.get() {
            self.done = true;
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
#[cfg(feature = "futures")]
impl FusedFuture for EventFuture {
    fn is_terminated(&self) -> bool {
        self.done
    }
}

struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
}
impl Task {
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
        Task {
            future: Box::pin(future),
        }
    }
    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(context)
    }
}

#[derive(Default)]
struct ExecutorInner {
    task_queue: RefCell<Vec<Task>>,
    new_tasks: RefCell<Vec<Task>>,
    events: RefCell<Slab<SharedBool>>,
}
impl ExecutorInner {
    fn release_event_handle(&self, event: &EventHandleInner) {
        self.events.borrow_mut().remove(event.index);
    }
}

/// Single-threaded polling-based executor
///
/// This is a thin-wrapper (using [`Rc`]) around the real executor, so that this struct can be
/// cloned and passed around.
///
/// See the [module documentation] for more details.
///
/// [module documentation]: index.html
#[derive(Clone, Default)]
pub struct Executor {
    inner: Rc<ExecutorInner>,
}
impl Executor {
    /// Spawn a new task to be run by this executor.
    ///
    /// # Example
    /// ```
    /// # use simple_async_local_executor::*;
    /// async fn nop() {}
    /// let executor = Executor::default();
    /// executor.spawn(nop());
    /// assert_eq!(executor.step(), false);
    /// ```
    pub fn spawn(&self, future: impl Future<Output = ()> + 'static) {
        self.inner.new_tasks.borrow_mut().push(Task::new(future));
    }
    /// Create an event handle, that can be used to [await](Executor::event()) and [notify](Executor::notify_event()) an event.
    pub fn create_event_handle(&self) -> EventHandle {
        let mut events = self.inner.events.borrow_mut();
        let index = events.insert(Rc::new(Cell::new(false)));
        EventHandle(Rc::new(EventHandleInner {
            index,
            executor: self.inner.clone(),
        }))
    }
    /// Notify an event.
    ///
    /// All tasks currently waiting on this event will
    /// progress at the next call to [`step`](Executor::step()).
    pub fn notify_event(&self, handle: &EventHandle) {
        self.inner.events.borrow_mut()[handle.0.index].replace(true);
    }
    /// Create an event future.
    ///
    /// Once this future is awaited, its task will be blocked until the next [`step`](Executor::step())
    /// after [`notify_event`](Executor::notify_event()) is called with this `handle`.
    pub fn event(&self, handle: &EventHandle) -> EventFuture {
        let ready = self.inner.events.borrow_mut()[handle.0.index].clone();
        EventFuture {
            ready,
            _handle: handle.clone(),
            done: false,
        }
    }
    /// Run each non-blocked task exactly once.
    ///
    /// Return whether there are any non-completed tasks.
    ///
    /// # Example
    /// ```
    /// # use simple_async_local_executor::*;
    /// let executor = Executor::default();
    /// let event = executor.create_event_handle();
    /// async fn wait_event(event: EventHandle, executor: Executor) {
    ///     executor.event(&event).await;
    /// }
    /// executor.spawn(wait_event(event.clone(), executor.clone()));
    /// assert_eq!(executor.step(), true); // still one task in the queue
    /// executor.notify_event(&event);
    /// assert_eq!(executor.step(), false); // no more task in the queue
    /// ```
    pub fn step(&self) -> bool {
        // dummy waker and context
        let waker = dummy_waker();
        let mut context = Context::from_waker(&waker);
        // append new tasks to all tasks
        let mut tasks = self.inner.task_queue.borrow_mut();
        tasks.append(&mut self.inner.new_tasks.borrow_mut());
        // go through all tasks, and keep uncompleted ones
        let mut uncompleted_tasks = Vec::new();
        let mut any_left = false;
        for mut task in tasks.drain(..) {
            match task.poll(&mut context) {
                Poll::Ready(()) => {} // task done
                Poll::Pending => {
                    uncompleted_tasks.push(task);
                    any_left = true;
                }
            }
        }
        // replace all tasks with uncompleted ones
        *tasks = uncompleted_tasks;
        // clear events
        for (_, event) in self.inner.events.borrow_mut().iter_mut() {
            event.replace(false);
        }
        any_left
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn nop() {}

    #[test]
    fn test_nop() {
        let executor = Executor::default();
        executor.spawn(nop());
        assert_eq!(executor.step(), false);
    }

    #[test]
    fn test_event() {
        let executor = Executor::default();
        let events = [
            executor.create_event_handle(),
            executor.create_event_handle(),
        ];

        async fn wait_event(events: [EventHandle; 2], executor: Executor) {
            println!("before awaits");
            executor.event(&events[0]).await;
            println!("between awaits");
            executor.event(&events[1]).await;
            println!("after awaits");
        }

        executor.spawn(wait_event(events.clone(), executor.clone()));
        println!("spawned");
        assert_eq!(executor.step(), true);
        assert_eq!(executor.inner.task_queue.borrow().len(), 1);
        println!("step 1");
        assert_eq!(executor.step(), true);
        println!("step 2");
        executor.notify_event(&events[0]);
        println!("notified 1");
        assert_eq!(executor.step(), true);
        executor.notify_event(&events[1]);
        println!("notified 2");
        assert_eq!(executor.step(), false);
        println!("step 3");
        assert_eq!(executor.inner.task_queue.borrow().len(), 0);
    }

    #[test]
    #[cfg(feature = "futures")]
    fn test_select() {
        use futures::select;
        let first_event_id = Rc::new(Cell::new(2));

        async fn wait_event(
            events: [EventHandle; 2],
            event_loop: Executor,
            first_event_id: Rc<Cell<usize>>,
        ) {
            println!("before select");
            let mut fut0 = event_loop.event(&events[0]);
            let mut fut1 = event_loop.event(&events[1]);
            select! {
                () = fut0 => { println!("event 0 fired first"); first_event_id.set(0); },
                () = fut1 => { println!("event 1 fired first"); first_event_id.set(1); }
            }
            println!("after select");
        }

        for i in 0..2 {
            println!("Testing event {}", i);
            let executor = Executor::default();
            {
                let events = [
                    executor.create_event_handle(),
                    executor.create_event_handle(),
                ];
                executor.spawn(wait_event(
                    events.clone(),
                    executor.clone(),
                    first_event_id.clone(),
                ));
                println!("spawned");
                assert_eq!(executor.step(), true);
                assert_eq!(executor.inner.task_queue.borrow().len(), 1);
                println!("step 1");
                assert_eq!(executor.step(), true);
                println!("step 2");
                executor.notify_event(&events[i]);
                println!("notified");
                assert_eq!(executor.step(), false);
                println!("step 3");
                assert_eq!(first_event_id.get(), i);
                assert_eq!(executor.inner.task_queue.borrow().len(), 0);
            }
            assert_eq!(executor.inner.events.borrow().len(), 0);
        }
    }
}
