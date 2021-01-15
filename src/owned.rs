use crate::{Handle, Node};

use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

pub struct Owned<T: Send + 'static> {
    node: NonNull<Node<T>>,
}

unsafe impl<T: Send> Send for Owned<T> {}

impl<T: Send> Owned<T> {
    pub fn new(handle: &Handle, data: T) -> Owned<T> {
        Owned {
            node: unsafe { NonNull::new_unchecked(Node::alloc(handle, data)) },
        }
    }
}

impl<T: Clone + Send + 'static> Clone for Owned<T> {
    fn clone(&self) -> Self {
        Owned {
            node: unsafe { NonNull::new_unchecked(Node::clone(self.node.as_ptr())) },
        }
    }
}

impl<T: Send> Deref for Owned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &self.node.as_ref().data }
    }
}

impl<T: Send> DerefMut for Owned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut self.node.as_mut().data }
    }
}

impl<T: Send> Drop for Owned<T> {
    fn drop(&mut self) {
        unsafe {
            Node::queue_drop(self.node.as_ptr());
        }
    }
}
