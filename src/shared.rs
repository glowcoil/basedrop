use crate::{Handle, Node};

use core::marker::PhantomData;
use core::ops::Deref;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering, fence};

pub struct Shared<T> {
    pub(crate) node: NonNull<Node<SharedInner<T>>>,
    pub(crate) phantom: PhantomData<SharedInner<T>>,
}

pub(crate) struct SharedInner<T> {
    count: AtomicUsize,
    data: T,
}

unsafe impl<T: Send + Sync> Send for Shared<T> {}
unsafe impl<T: Send + Sync> Sync for Shared<T> {}

impl<T: Send + 'static> Shared<T> {
    pub fn new(handle: &Handle, data: T) -> Shared<T> {
        Shared {
            node: unsafe {
                NonNull::new_unchecked(Node::alloc(handle, SharedInner {
                    count: AtomicUsize::new(1),
                    data,
                }))
            },
            phantom: PhantomData,
        }
    }
}

impl<T> Shared<T> {
    pub fn get_mut(this: &mut Self) -> Option<&mut T> {
        unsafe {
            if this.node.as_ref().data.count.load(Ordering::Acquire) == 1 {
                Some(&mut this.node.as_mut().data.data)
            } else {
                None
            }
        }
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        unsafe {
            self.node.as_ref().data.count.fetch_add(1, Ordering::Relaxed);
        }

        Shared { node: self.node, phantom: PhantomData }
    }
}

impl<T> Deref for Shared<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &self.node.as_ref().data.data }
    }
}

impl<T> Drop for Shared<T> {
    fn drop(&mut self) {
        unsafe {
            let count = self.node.as_ref().data.count.fetch_sub(1, Ordering::Release);

            if count == 1 {
                fence(Ordering::Acquire);
                Node::queue_drop(self.node.as_ptr());
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
