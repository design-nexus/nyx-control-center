//! A value that views subscribe to while they're shown.
//!
//! The first subscriber starts the source (a D-Bus subscription, a `pactl subscribe`
//! process, a timer); the last one leaving stops it. Single-threaded: everything runs on
//! the main thread, and sources hand their results back to it.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

type Callback<T> = Rc<dyn Fn(&T)>;
type Starter<T> = Box<dyn Fn(Setter<T>) -> Box<dyn Stop>>;

/// What a running source keeps; dropping it (or calling `stop`) ends the source.
pub trait Stop {
    fn stop(self: Box<Self>) {}
}

impl Stop for () {}

/// Stops by running a closure.
pub struct OnStop(pub Option<Box<dyn FnOnce()>>);

impl Stop for OnStop {
    fn stop(mut self: Box<Self>) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

struct Inner<T: 'static> {
    value: RefCell<T>,
    subscribers: RefCell<Vec<(u64, Callback<T>)>>,
    next_id: Cell<u64>,
    start: Starter<T>,
    running: RefCell<Option<Box<dyn Stop>>>,
}

/// A live value. Cloning shares it.
pub struct Live<T: 'static> {
    inner: Rc<Inner<T>>,
}

impl<T: 'static> Clone for Live<T> {
    fn clone(&self) -> Self {
        Live { inner: self.inner.clone() }
    }
}

/// How a source publishes new values. Holds the value weakly: a source that outlives
/// its last subscriber publishes into nothing.
pub struct Setter<T: 'static> {
    inner: Weak<Inner<T>>,
}

impl<T: 'static> Clone for Setter<T> {
    fn clone(&self) -> Self {
        Setter { inner: self.inner.clone() }
    }
}

impl<T: Clone + PartialEq + 'static> Setter<T> {
    /// Publish a value; subscribers hear about it only if it changed.
    pub fn set(&self, value: T) {
        let Some(inner) = self.inner.upgrade() else { return };
        if *inner.value.borrow() == value {
            return;
        }
        *inner.value.borrow_mut() = value;
        notify(&inner);
    }

    /// Change part of the value.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        let Some(inner) = self.inner.upgrade() else { return };
        let mut next = inner.value.borrow().clone();
        f(&mut next);
        drop(inner);
        self.set(next);
    }

    pub fn get(&self) -> Option<T> {
        self.inner.upgrade().map(|i| i.value.borrow().clone())
    }
}

fn notify<T: 'static>(inner: &Rc<Inner<T>>) {
    // Copy the list first: a callback may subscribe or unsubscribe.
    let subs: Vec<Callback<T>> = inner.subscribers.borrow().iter().map(|(_, f)| f.clone()).collect();
    let value = inner.value.borrow();
    for f in subs {
        f(&value);
    }
}

/// Ends a subscription when dropped.
#[must_use = "the subscription ends when this is dropped"]
pub struct Subscription {
    end: Option<Box<dyn FnOnce()>>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(end) = self.end.take() {
            end();
        }
    }
}

impl<T: Clone + PartialEq + 'static> Live<T> {
    /// A value that starts as `initial`; `start` runs when the first view subscribes and
    /// returns what stops it again.
    pub fn new(initial: T, start: impl Fn(Setter<T>) -> Box<dyn Stop> + 'static) -> Self {
        Live {
            inner: Rc::new(Inner {
                value: RefCell::new(initial),
                subscribers: RefCell::default(),
                next_id: Cell::new(0),
                start: Box::new(start),
                running: RefCell::default(),
            }),
        }
    }

    /// A value that never changes by itself (set it through [`Live::setter`]).
    pub fn fixed(value: T) -> Self {
        Live::new(value, |_| Box::new(()))
    }

    pub fn get(&self) -> T {
        self.inner.value.borrow().clone()
    }

    pub fn setter(&self) -> Setter<T> {
        Setter { inner: Rc::downgrade(&self.inner) }
    }

    pub fn running(&self) -> bool {
        self.inner.running.borrow().is_some()
    }

    /// Call `f` now with the current value and again whenever it changes, until the
    /// returned subscription is dropped.
    pub fn subscribe(&self, f: impl Fn(&T) + 'static) -> Subscription {
        let id = self.inner.next_id.get();
        self.inner.next_id.set(id + 1);
        let f: Callback<T> = Rc::new(f);
        self.inner.subscribers.borrow_mut().push((id, f.clone()));
        if self.inner.running.borrow().is_none() {
            let stop = (self.inner.start)(self.setter());
            *self.inner.running.borrow_mut() = Some(stop);
        }
        f(&self.inner.value.borrow());
        let weak = Rc::downgrade(&self.inner);
        Subscription {
            end: Some(Box::new(move || {
                let Some(inner) = weak.upgrade() else { return };
                inner.subscribers.borrow_mut().retain(|(i, _)| *i != id);
                if inner.subscribers.borrow().is_empty() {
                    let running = inner.running.borrow_mut().take();
                    if let Some(stop) = running {
                        stop.stop();
                    }
                }
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Counted = (Live<i32>, Rc<Cell<u32>>, Rc<Cell<u32>>, Rc<RefCell<Option<Setter<i32>>>>);

    fn counted() -> Counted {
        let (starts, stops) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
        let setter: Rc<RefCell<Option<Setter<i32>>>> = Rc::default();
        let (s1, s2, st) = (starts.clone(), stops.clone(), setter.clone());
        let live = Live::new(0, move |set| {
            s1.set(s1.get() + 1);
            *st.borrow_mut() = Some(set);
            let s2 = s2.clone();
            Box::new(OnStop(Some(Box::new(move || s2.set(s2.get() + 1)))))
        });
        (live, starts, stops, setter)
    }

    #[test]
    fn starts_with_the_first_view_and_stops_with_the_last() {
        let (live, starts, stops, _) = counted();
        assert!(!live.running());
        let a = live.subscribe(|_| {});
        let b = live.subscribe(|_| {});
        assert_eq!((starts.get(), live.running()), (1, true));
        drop(a);
        assert_eq!(stops.get(), 0);
        drop(b);
        assert_eq!((stops.get(), live.running()), (1, false));
        let _c = live.subscribe(|_| {});
        assert_eq!(starts.get(), 2, "starts again for a new view");
    }

    #[test]
    fn views_hear_changes_only() {
        let (live, _, _, setter) = counted();
        let seen: Rc<RefCell<Vec<i32>>> = Rc::default();
        let s = seen.clone();
        let _sub = live.subscribe(move |v| s.borrow_mut().push(*v));
        let set = setter.borrow().clone().unwrap();
        set.set(5);
        set.set(5);
        set.update(|v| *v += 1);
        assert_eq!(*seen.borrow(), vec![0, 5, 6], "the current value first, then each change");
        assert_eq!(live.get(), 6);
    }

    #[test]
    fn a_source_outliving_its_value_is_harmless() {
        let (live, _, _, setter) = counted();
        let sub = live.subscribe(|_| {});
        let set = setter.borrow().clone().unwrap();
        drop(sub);
        drop(live);
        set.set(9);
        assert_eq!(set.get(), None);
    }
}
