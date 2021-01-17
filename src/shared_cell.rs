use crate::{Node, Shared, SharedInner};

use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

pub struct SharedCell<T: Send + 'static> {
    readers: AtomicUsize,
    node: AtomicPtr<Node<SharedInner<T>>>,
}

unsafe impl<T: Send + Sync> Send for SharedCell<T> {}
unsafe impl<T: Send + Sync> Sync for SharedCell<T> {}

impl<T: Send + 'static> SharedCell<T> {
    pub fn new(value: Shared<T>) -> SharedCell<T> {
        SharedCell {
            readers: AtomicUsize::new(0),
            node: AtomicPtr::new(value.node),
        }
    }

    pub fn get(&self) -> Shared<T> {
        self.readers.fetch_add(1, Ordering::SeqCst);
        let node = self.node.load(Ordering::SeqCst);
        self.readers.fetch_sub(1, Ordering::Relaxed);
        Shared { node }
    }

    pub fn set(&self, value: Shared<T>) {
        let old = self.replace(value);
        core::mem::drop(old);
    }

    pub fn replace(&self, value: Shared<T>) -> Shared<T> {
        let old = self.node.swap(value.node, Ordering::AcqRel);
        while self.readers.load(Ordering::Relaxed) != 0 {}
        Shared { node: old }
    }

    pub fn into_inner(self) -> Shared<T> {
        Shared { node: self.node.into_inner() }
    }
}
