use crate::{Handle, Node};

use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicUsize, Ordering};

pub struct Shared<T: Send + 'static> {
    node: *mut Node<SharedInner<T>>,
}

struct SharedInner<T: Send + 'static> {
    count: AtomicUsize,
    data: T,
}

unsafe impl<T: Send + Sync> Send for Shared<T> {}
unsafe impl<T: Send + Sync> Sync for Shared<T> {}

impl<T: Send + 'static> Shared<T> {
    pub fn new(handle: &Handle, data: T) -> Shared<T> {
        Shared {
            node: unsafe {
                Node::alloc(
                    handle,
                    SharedInner {
                        count: AtomicUsize::new(1),
                        data,
                    },
                )
            },
        }
    }
}

impl<T: Send> Clone for Shared<T> {
    fn clone(&self) -> Self {
        unsafe {
            (*self.node).data.count.fetch_add(1, Ordering::Relaxed);
        }

        Shared { node: self.node }
    }
}

impl<T: Send> Deref for Shared<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.node).data.data }
    }
}

impl<T: Send> DerefMut for Shared<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut (*self.node).data.data }
    }
}

impl<T: Send> Drop for Shared<T> {
    fn drop(&mut self) {
        unsafe {
            let count = (*self.node).data.count.fetch_sub(1, Ordering::Release);

            if count == 1 {
                Node::queue_drop(self.node);
            }
        }
    }
}

#[test]
fn test() {
    use crate::Collector;

    extern crate alloc;
    use alloc::sync::Arc;

    struct Test(Arc<AtomicUsize>);

    impl Drop for Test {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    let counter = Arc::new(AtomicUsize::new(0));

    let mut collector = Collector::new();
    let handle = collector.handle();

    let shared = Shared::new(&handle, Test(counter.clone()));
    let mut copies = alloc::vec::Vec::new();
    for _ in 0..10 {
        copies.push(shared.clone());
    }

    assert_eq!(counter.load(Ordering::Relaxed), 0);

    core::mem::drop(shared);
    core::mem::drop(copies);
    collector.collect();

    assert_eq!(counter.load(Ordering::Relaxed), 1);
}
