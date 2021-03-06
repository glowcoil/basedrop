use crate::{Node, Shared, SharedInner};

use core::marker::PhantomData;
use core::ptr::NonNull;
use core::sync::atomic::{fence, AtomicPtr, AtomicUsize, Ordering};

/// A thread-safe shared mutable memory location that holds a [`Shared<T>`].
///
/// `SharedCell` is designed to be low-overhead for readers at the expense of
/// somewhat higher overhead for writers.
///
/// [`Shared<T>`]: crate::Shared
pub struct SharedCell<T> {
    readers: AtomicUsize,
    node: AtomicPtr<Node<SharedInner<T>>>,
    phantom: PhantomData<Shared<T>>,
}

unsafe impl<T: Send + Sync> Send for SharedCell<T> {}
unsafe impl<T: Send + Sync> Sync for SharedCell<T> {}

impl<T: Send + 'static> SharedCell<T> {
    /// Constructs a new `SharedCell` containing `value`.
    ///
    /// # Examples
    /// ```
    /// use basedrop::{Collector, Shared, SharedCell};
    ///
    /// let collector = Collector::new();
    /// let three = Shared::new(&collector.handle(), 3);
    /// let cell = SharedCell::new(three);
    /// ```
    pub fn new(value: Shared<T>) -> SharedCell<T> {
        let node = value.node.as_ptr();
        core::mem::forget(value);

        SharedCell {
            readers: AtomicUsize::new(0),
            node: AtomicPtr::new(node),
            phantom: PhantomData,
        }
    }
}

impl<T> SharedCell<T> {
    /// Gets a copy of the contained [`Shared<T>`], incrementing its reference
    /// count in the process.
    ///
    /// # Examples
    /// ```
    /// use basedrop::{Collector, Shared, SharedCell};
    ///
    /// let collector = Collector::new();
    /// let x = Shared::new(&collector.handle(), 3);
    /// let cell = SharedCell::new(x);
    ///
    /// let y = cell.get();
    /// ```
    ///
    /// [`Shared<T>`]: crate::Shared
    pub fn get(&self) -> Shared<T> {
        self.readers.fetch_add(1, Ordering::SeqCst);

        let shared = Shared {
            node: unsafe { NonNull::new_unchecked(self.node.load(Ordering::SeqCst)) },
            phantom: PhantomData,
        };
        let copy = shared.clone();
        core::mem::forget(shared);

        self.readers.fetch_sub(1, Ordering::Relaxed);

        copy
    }

    /// Replaces the contained [`Shared<T>`], decrementing its reference count
    /// in the process.
    ///
    /// # Examples
    /// ```
    /// use basedrop::{Collector, Shared, SharedCell};
    ///
    /// let collector = Collector::new();
    /// let x = Shared::new(&collector.handle(), 3);
    /// let cell = SharedCell::new(x);
    ///
    /// let y = Shared::new(&collector.handle(), 4);
    /// cell.set(y);
    /// ```
    ///
    /// [`Shared<T>`]: crate::Shared
    pub fn set(&self, value: Shared<T>) {
        let old = self.replace(value);
        core::mem::drop(old);
    }

    /// Replaces the contained [`Shared<T>`] and returns it.
    ///
    /// # Examples
    /// ```
    /// use basedrop::{Collector, Shared, SharedCell};
    ///
    /// let collector = Collector::new();
    /// let x = Shared::new(&collector.handle(), 3);
    /// let cell = SharedCell::new(x);
    ///
    /// let y = Shared::new(&collector.handle(), 4);
    /// let x = cell.replace(y);
    /// ```
    ///
    /// [`Shared<T>`]: crate::Shared
    pub fn replace(&self, value: Shared<T>) -> Shared<T> {
        let node = value.node.as_ptr();
        core::mem::forget(value);

        let old = self.node.swap(node, Ordering::AcqRel);
        while self.readers.load(Ordering::Relaxed) != 0 {}
        fence(Ordering::Acquire);

        Shared {
            node: unsafe { NonNull::new_unchecked(old) },
            phantom: PhantomData,
        }
    }

    /// Consumes the `SharedCell` and returns the contained [`Shared<T>`]. This
    /// is safe because we are guaranteed to be the only holder of the
    /// `SharedCell`.
    ///
    /// # Examples
    /// ```
    /// use basedrop::{Collector, Shared, SharedCell};
    ///
    /// let collector = Collector::new();
    /// let x = Shared::new(&collector.handle(), 3);
    /// let cell = SharedCell::new(x);
    ///
    /// let x = cell.into_inner();
    /// ```
    ///
    /// [`Shared<T>`]: crate::Shared
    pub fn into_inner(mut self) -> Shared<T> {
        let node = core::mem::replace(&mut self.node, AtomicPtr::new(core::ptr::null_mut()));
        core::mem::forget(self);
        Shared {
            node: unsafe { NonNull::new_unchecked(node.into_inner()) },
            phantom: PhantomData,
        }
    }
}

impl<T> Drop for SharedCell<T> {
    fn drop(&mut self) {
        let _ = Shared {
            node: unsafe { NonNull::new_unchecked(self.node.load(Ordering::Relaxed)) },
            phantom: PhantomData,
        };
    }
}

#[cfg(test)]
mod tests {
    use crate::{Collector, Shared, SharedCell};

    use core::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn shared_cell() {
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
        let shared = Shared::new(&collector.handle(), Test(counter.clone()));
        let cell = SharedCell::new(shared);
        collector.collect();

        assert_eq!(counter.load(Ordering::Relaxed), 0);

        let copy = cell.get();
        let copy2 = cell.replace(copy);
        collector.collect();

        assert_eq!(counter.load(Ordering::Relaxed), 0);

        core::mem::drop(cell);
        collector.collect();

        assert_eq!(counter.load(Ordering::Relaxed), 0);

        core::mem::drop(copy2);
        collector.collect();

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }
}
