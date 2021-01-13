#![no_std]

use core::sync::atomic::{AtomicPtr, Ordering};

extern crate alloc;
use alloc::boxed::Box;
use alloc::sync::Arc;

struct Node {
    next: AtomicPtr<Node>,
}

struct Collector {
    head: *mut Node,
    tail: Arc<AtomicPtr<Node>>,
    stub: *mut Node,
}

impl Collector {
    pub fn new() -> Collector {
        let head = Box::into_raw(Box::new(Node {
            next: AtomicPtr::new(core::ptr::null_mut()),
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
                    let _ = Box::from_raw(head);
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct Handle {
    tail: Arc<AtomicPtr<Node>>,
}

impl Handle {
    pub fn push(&self) {
        let node = Box::into_raw(Box::new(Node {
            next: AtomicPtr::new(core::ptr::null_mut()),
        }));

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

    impl Collector {
        fn len(&self) -> usize {
            let mut len = 0;
            let mut node = self.head;
            loop {
                let next = unsafe { (*node).next.load(Ordering::Acquire) };
                if next.is_null() {
                    break;
                }  else {
                    len += 1;
                    node = next;
                }
            }
            len
        }
    }

    #[test]
    fn single_threaded() {
        let mut collector = Collector::new();
        let handle = collector.handle();
        for _ in 0..100 {
            handle.push();
        }

        let tail = collector.tail.load(Ordering::Relaxed);
        assert!(collector.head != tail);
        assert!(collector.len() == 100);

        collector.collect();

        let tail = collector.tail.load(Ordering::Relaxed);
        assert!(collector.head == tail);
        assert!(collector.head == collector.stub);
        assert!(collector.len() == 0);
    }

    #[test]
    fn multi_threaded() {
        let mut collector = Collector::new();
        let handle = collector.handle();
        let mut threads = alloc::vec![];
        for _ in 0..100 {
            let handle = handle.clone();
            threads.push(std::thread::spawn(move || {
                for _ in 0..100 {
                    handle.push();
                }
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }

        let tail = collector.tail.load(Ordering::Relaxed);
        assert!(collector.head != tail);
        assert!(collector.len() == 10000);

        collector.collect();

        let tail = collector.tail.load(Ordering::Relaxed);
        assert!(collector.head == tail);
        assert!(collector.head == collector.stub);
        assert!(collector.len() == 0);
    }

    #[test]
    fn simultaneous_push_and_collect() {
        let mut collector = Collector::new();
        let handle = collector.handle();
        let mut threads = alloc::vec![];
        for _ in 0..100 {
            let handle = handle.clone();
            threads.push(std::thread::spawn(move || {
                for _ in 0..100 {
                    handle.push();
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
        assert!(collector.len() == 0);
    }
}
