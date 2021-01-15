use crate::{Handle, Node};

use core::ops::{Deref, DerefMut};

pub struct Owned<T: Send> {
    node: *mut Node<T>,
}

unsafe impl<T: Send> Send for Owned<T> {}

impl<T: Send> Owned<T> {
    pub fn new(handle: &Handle, data: T) -> Owned<T> {
        Owned {
            node: unsafe { Node::alloc(handle, data) },
        }
    }
}

impl<T: Send> Deref for Owned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.node).data }
    }
}

impl<T: Send> DerefMut for Owned<T> {
    fn deref_mut (&mut self) -> &mut Self::Target {
        unsafe { &mut (*self.node).data }
    }
}

impl<T: Send> Drop for Owned<T> {
    fn drop(&mut self) {
        unsafe {
            Node::queue_drop(self.node);
        }
    }
}
