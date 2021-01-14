#![no_std]

use core::sync::atomic::{AtomicPtr, Ordering};

extern crate alloc;
use alloc::boxed::Box;
use alloc::sync::Arc;

struct Node<T: Send> {
    next: AtomicPtr<Node<()>>,
    drop: unsafe fn(*mut Node<()>),
    data: T,
}

unsafe fn drop_node<T: Send>(node: *mut Node<()>) {
    let _ = Box::from_raw(node as *mut Node<T>);
}

struct Collector {
    head: *mut Node<()>,
    tail: Arc<AtomicPtr<Node<()>>>,
    stub: *mut Node<()>,
}

unsafe impl Send for Collector {}

impl Collector {
    pub fn new() -> Collector {
        let head = Box::into_raw(Box::new(Node {
            next: AtomicPtr::new(core::ptr::null_mut()),
            drop: drop_node::<()>,
            data: (),
        }));

        Collector {
            head,
            tail: Arc::new(AtomicPtr::new(head)),
            stub: head,
        }
    }

    pub fn handle(&self) -> Handle {
        Handle {
            tail: self.tail.clone(),
        }
    }

    fn collect(&mut self) {
        loop {
            unsafe {
                let next = (*self.head).next.load(Ordering::Acquire);
                if next.is_null() {
                    break;
                }

                let head = self.head;
                self.head = next;
                if head == self.stub {
                    (*head).next.store(core::ptr::null_mut(), Ordering::Release);
                    let tail = self.tail.swap(head, Ordering::AcqRel);
                    (*tail).next.store(head, Ordering::Release);
                } else {
                    ((*head).drop)(head);
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct Handle {
    tail: Arc<AtomicPtr<Node<()>>>,
}

impl Handle {
    pub fn push<T: Send>(&self, data: T) {
        let node = Box::into_raw(Box::new(Node {
            next: AtomicPtr::new(core::ptr::null_mut()),
            drop: drop_node::<T>,
            data,
        })) as *mut Node<()>;

        let tail = self.tail.swap(node, Ordering::AcqRel);
        unsafe {
            (*tail).next.store(node, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    extern crate std;

    use core::sync::atomic::AtomicUsize;

    struct Test(Arc<AtomicUsize>);

    impl Drop for Test {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn collector() {
        let counter = Arc::new(AtomicUsize::new(0));

        let mut collector = Collector::new();
        let handle = collector.handle();
        let mut threads = alloc::vec![];
        for _ in 0..100 {
            let handle = handle.clone();
            let counter = counter.clone();
            threads.push(std::thread::spawn(move || {
                for _ in 0..100 {
                    handle.push(Test(counter.clone()));
                }
            }));
        }

        for _ in 0..100 {
            collector.collect();
        }

        for thread in threads {
            thread.join().unwrap();
        }

        collector.collect();

        let tail = collector.tail.load(Ordering::Relaxed);
        assert!(collector.head == tail);
        assert!(collector.head == collector.stub);
        let next = unsafe { (*collector.head).next.load(Ordering::Relaxed) };
        assert!(next.is_null());

        assert!(counter.load(Ordering::Relaxed) == 10000);
    }
}
