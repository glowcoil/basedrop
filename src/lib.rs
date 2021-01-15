#![no_std]

use core::sync::atomic::{AtomicPtr, Ordering};

extern crate alloc;
use alloc::boxed::Box;
use alloc::sync::Arc;

pub struct Node<T> {
    tail: Arc<AtomicPtr<Node<()>>>,
    next: AtomicPtr<Node<()>>,
    drop: unsafe fn(*mut Node<()>),
    pub data: T,
}

unsafe fn drop_node<T: Send>(node: *mut Node<()>) {
    let _ = Box::from_raw(node as *mut Node<T>);
}

impl<T: Send> Node<T> {
    pub unsafe fn alloc(handle: &Handle, data: T) -> *mut Node<T> {
        Box::into_raw(Box::new(Node {
            tail: handle.tail.clone(),
            next: AtomicPtr::new(core::ptr::null_mut()),
            drop: drop_node::<T>,
            data,
        }))
    }

    pub unsafe fn queue_drop(node: *mut Node<T>) {
        let tail = (*node).tail.swap(node as *mut Node<()>, Ordering::AcqRel);
        (*tail).next.store(node as *mut Node<()>, Ordering::Release);
    }
}

#[derive(Clone)]
pub struct Handle {
    tail: Arc<AtomicPtr<Node<()>>>,
}

pub struct Collector {
    head: *mut Node<()>,
    tail: Arc<AtomicPtr<Node<()>>>,
    stub: *mut Node<()>,
}

unsafe impl Send for Collector {}

impl Collector {
    pub fn new() -> Collector {
        let tail = Arc::new(AtomicPtr::new(core::ptr::null_mut()));
        let head = Box::into_raw(Box::new(Node {
            tail: tail.clone(),
            next: AtomicPtr::new(core::ptr::null_mut()),
            drop: drop_node::<()>,
            data: (),
        }));
        tail.store(head, Ordering::Release);

        Collector {
            head,
            tail: tail,
            stub: head,
        }
    }

    pub fn handle(&self) -> Handle {
        Handle {
            tail: self.tail.clone(),
        }
    }

    pub fn collect(&mut self) {
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
                    let node = unsafe { Node::alloc(&handle, Test(counter.clone())) };
                    unsafe {
                        Node::queue_drop(node);
                    }
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
