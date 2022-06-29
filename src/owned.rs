use crate::{Handle, Node};

use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

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

#[cfg(feature = "stable_deref_trait")]
unsafe impl<T> stable_deref_trait::StableDeref for Owned<T> {}

impl<T> Drop for Owned<T> {
    fn drop(&mut self) {
        unsafe {
            Node::queue_drop(self.node.as_ptr());
        }
    }
}
