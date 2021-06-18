use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

use crate::{Handle, Node};

/// An owned smart pointer with deferred collection, analogous to `Box`.
///
/// When an `Owned<T>` is dropped, its contents are added to the drop queue
/// of the [`Collector`] whose [`Handle`] it was originally allocated with.
/// As the collector may be on another thread, contents are required to be
/// `Send + 'static`.
///
/// [`Collector`]: crate::Collector
/// [`Handle`]: crate::Handle
pub struct Owned<T> {
    node: NonNull<Node<T>>,
    phantom: PhantomData<T>,
}

unsafe impl<T: Send> Send for Owned<T> {}
unsafe impl<T: Sync> Sync for Owned<T> {}

impl<T: Send + 'static> Owned<T> {
    /// Constructs a new `Owned<T>`.
    ///
    /// # Examples
    /// ```
    /// use basedrop::{Collector, Owned};
    ///
    /// let collector = Collector::new();
    /// let three = Owned::new(&collector.handle(), 3);
    /// ```
    pub fn new(handle: &Handle, data: T) -> Owned<T> {
        Owned {
            node: unsafe { NonNull::new_unchecked(Node::alloc(handle, data)) },
            phantom: PhantomData,
        }
    }
}

impl<T: Clone + Send + 'static> Clone for Owned<T> {
    fn clone(&self) -> Self {
        let handle = unsafe { Node::handle(self.node.as_ptr()) };
        Owned::new(&handle, self.deref().clone())
    }
}

impl<T> Deref for Owned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &self.node.as_ref().data }
    }
}

impl<T> DerefMut for Owned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut self.node.as_mut().data }
    }
}

impl<T> Drop for Owned<T> {
    fn drop(&mut self) {
        unsafe {
            Node::queue_drop(self.node.as_ptr());
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::{Arc, Mutex};

    use crate::Collector;

    use super::*;

    #[test]
    fn test_we_can_create_an_owned() {
        struct Test;
        let collector = Collector::new();
        let _owned = Owned::new(&collector.handle(), Test{});
    }

    #[test]
    fn test_we_can_destruct_an_owned_and_it_will_be_dropped() {
        struct Test(Arc<Mutex<bool>>);
        impl Drop for Test {
            fn drop(&mut self) {
                let mut lock = self.0.lock().unwrap();
                *lock = true;
            }
        }
        let mut collector = Collector::new();
        let has_dropped = Arc::new(Mutex::new(false));
        {
            let _owned = Owned::new(&collector.handle(), Test(has_dropped.clone()));
        }
        assert_eq!(*has_dropped.lock().unwrap().deref(), false);
        collector.collect();
        assert_eq!(*has_dropped.lock().unwrap().deref(), true);
    }
}