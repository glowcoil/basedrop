#![no_std]

use core::mem::ManuallyDrop;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

extern crate alloc;
use alloc::boxed::Box;

union NodeLink {
    tail: *mut AtomicPtr<Node<()>>,
    next: ManuallyDrop<AtomicPtr<Node<()>>>,
}

pub struct Node<T> {
    link: NodeLink,
    drop: unsafe fn(*mut Node<()>),
    pub data: T,
}

unsafe fn drop_node<T: Send>(node: *mut Node<()>) {
    let _ = Box::from_raw(node as *mut Node<T>);
}

impl<T: Send> Node<T> {
    pub unsafe fn alloc(handle: &Handle, data: T) -> *mut Node<T> {
        (*handle.inner).allocs.fetch_add(1, Ordering::Relaxed);

        Box::into_raw(Box::new(Node {
            link: NodeLink {
                tail: &mut (*handle.inner).tail,
            },
            drop: drop_node::<T>,
            data,
        }))
    }

    pub unsafe fn queue_drop(node: *mut Node<T>) {
        let tail = (*node).link.tail;
        (*node).link.next = ManuallyDrop::new(AtomicPtr::new(core::ptr::null_mut()));
        let tail = (*tail).swap(node as *mut Node<()>, Ordering::AcqRel);
        (*tail).link.next.store(node as *mut Node<()>, Ordering::Release);
    }
}

pub struct Handle {
    inner: *mut CollectorInner,
}

unsafe impl Send for Handle {}

impl Clone for Handle {
    fn clone(&self) -> Self {
        unsafe {
            (*self.inner).handles.fetch_add(1, Ordering::Relaxed);
        }

        Handle { inner: self.inner }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            (*self.inner).handles.fetch_sub(1, Ordering::Release);
        }
    }
}

struct CollectorInner {
    handles: AtomicUsize,
    allocs: AtomicUsize,
    tail: AtomicPtr<Node<()>>,
}

pub struct Collector {
    head: *mut Node<()>,
    stub: *mut Node<()>,
    inner: *mut CollectorInner,
}

unsafe impl Send for Collector {}

impl Collector {
    pub fn new() -> Collector {
        let head = Box::into_raw(Box::new(Node {
            link: NodeLink {
                next: ManuallyDrop::new(AtomicPtr::new(core::ptr::null_mut())),
            },
            drop: drop_node::<()>,
            data: (),
        }));

        let inner = Box::into_raw(Box::new(CollectorInner {
            handles: AtomicUsize::new(0),
            allocs: AtomicUsize::new(0),
            tail: AtomicPtr::new(head),
        }));

        Collector {
            head,
            stub: head,
            inner,
        }
    }

    pub fn handle(&self) -> Handle {
        unsafe {
            (*self.inner).handles.fetch_add(1, Ordering::Relaxed);
        }

        Handle {
            inner: self.inner,
        }
    }

    pub fn collect(&mut self) {
        loop {
            unsafe {
                let next = (*self.head).link.next.load(Ordering::Acquire);
                if next.is_null() {
                    break;
                }

                let head = self.head;
                self.head = next;
                if head == self.stub {
                    (*head).link.next.store(core::ptr::null_mut(), Ordering::Release);
                    let tail = (*self.inner).tail.swap(head, Ordering::AcqRel);
                    (*tail).link.next.store(head, Ordering::Release);
                } else {
                    ((*head).drop)(head);
                    (*self.inner).allocs.fetch_sub(1, Ordering::Release);
                }
            }
        }
    }

    pub fn try_cleanup(self) -> Result<(), Self> {
        unsafe {
            let handles = (*self.inner).handles.load(Ordering::Acquire);
            if handles == 0 {
                let allocs = (*self.inner).allocs.load(Ordering::Acquire);
                if allocs == 0 {
                    let _ = Box::from_raw(self.stub);
                    let _ = Box::from_raw(self.inner);

                    return Ok(());
                }
            }
        }

        Err(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    extern crate std;

    use alloc::sync::Arc;
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

        let collector = Collector::new();
        let handle = collector.handle();

        let node = unsafe { Node::alloc(&handle, ()) };
        let result = collector.try_cleanup();
        assert!(result.is_err());
        let mut collector = result.unwrap_err();
        unsafe {
            Node::queue_drop(node);
        }

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

        let tail = unsafe { (*collector.inner).tail.load(Ordering::Relaxed) };
        assert!(collector.head == tail);
        assert!(collector.head == collector.stub);
        let next = unsafe { (*collector.head).link.next.load(Ordering::Relaxed) };
        assert!(next.is_null());

        assert!(counter.load(Ordering::Relaxed) == 10000);

        core::mem::drop(handle);

        let result = collector.try_cleanup();
        assert!(result.is_ok());
    }
}
