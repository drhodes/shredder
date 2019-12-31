use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt::{self, Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;

use crate::collector::{GcInternalHandle, COLLECTOR};
use crate::Scan;

#[derive(Debug)]
pub struct Gc<T: Scan> {
    backing_handle: GcInternalHandle,
    direct_ptr: *const T,
}

impl<T: Scan> Gc<T> {
    pub fn new(v: T) -> Self
    where
        T: 'static,
    {
        let (handle, ptr) = COLLECTOR.lock().track_data(v);
        Self {
            backing_handle: handle,
            direct_ptr: ptr,
        }
    }

    #[must_use]
    pub fn get(&self) -> GcGuard<T> {
        let mut locked_collector = COLLECTOR.lock();
        if !locked_collector.validate_handle(&self.backing_handle) {
            drop(locked_collector);
            panic!("Tried to access Gc data, but the internal state was corrupted (perhaps you're manipulating Gc<?> in a destructor?)");
        }

        locked_collector.inc_held_references();
        GcGuard { gc_ptr: self }
    }

    pub(crate) fn internal_handle(&self) -> GcInternalHandle {
        self.backing_handle.clone()
    }
}

impl<T: Scan> Clone for Gc<T> {
    #[must_use]
    fn clone(&self) -> Self {
        let mut locked_collector = COLLECTOR.lock();
        if !locked_collector.validate_handle(&self.backing_handle) {
            drop(locked_collector);
            panic!("Tried to clone a Gc, but the internal state was corrupted (perhaps you're manipulating Gc<?> in a destructor?)");
        }

        let new_handle = locked_collector.clone_handle(&self.backing_handle);

        Self {
            backing_handle: new_handle,
            direct_ptr: self.direct_ptr,
        }
    }
}

// Same bounds as Arc<T>
unsafe impl<T: Scan> Sync for Gc<T> where T: Sync + Send {}
unsafe impl<T: Scan> Send for Gc<T> where T: Sync + Send {}
// Since we can clone Gc<T>, being able to send a Gc<T> implies possible sharing between threads
// (Thus for Gc<T> to be send, T must be Send and Sync)

impl<T: Scan> Drop for Gc<T> {
    fn drop(&mut self) {
        // This may trigger during Gc-drop, but it'll do nothing and everything will be fine
        COLLECTOR.lock().drop_handle(&self.backing_handle);
    }
}

// Lots of traits it's good for a smart ptr to implement:
impl<T: Scan> Default for Gc<T>
where
    T: Default + 'static,
{
    #[must_use]
    fn default() -> Self {
        let v = T::default();
        Self::new(v)
    }
}

impl<T: Scan> Display for Gc<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let a = self.get();
        a.fmt(f)
    }
}

impl<T: Scan> fmt::Pointer for Gc<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.direct_ptr, f)
    }
}

impl<T: Scan> Eq for Gc<T> where T: Eq {}

impl<T: Scan> Hash for Gc<T>
where
    T: Hash,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.get().hash(state)
    }
}

impl<T: Scan> Ord for Gc<T>
where
    T: Ord,
{
    #[must_use]
    fn cmp(&self, other: &Self) -> Ordering {
        let a = self.get();
        let b = other.get();

        a.cmp(b.deref())
    }
}

#[allow(clippy::partialeq_ne_impl)]
impl<T: Scan> PartialEq for Gc<T>
where
    T: PartialEq,
{
    #[must_use]
    fn eq(&self, other: &Self) -> bool {
        let a = self.get();
        let b = other.get();
        a.eq(&b)
    }

    #[must_use]
    fn ne(&self, other: &Self) -> bool {
        let a = self.get();
        let b = other.get();
        a.ne(&b)
    }
}

impl<T: Scan> PartialOrd for Gc<T>
where
    T: PartialOrd,
{
    #[must_use]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let a = self.get();
        let b = other.get();

        a.partial_cmp(&b)
    }

    #[must_use]
    fn lt(&self, other: &Self) -> bool {
        let a = self.get();
        let b = other.get();

        a.lt(&b)
    }

    #[must_use]
    fn le(&self, other: &Self) -> bool {
        let a = self.get();
        let b = other.get();

        a.le(&b)
    }

    #[must_use]
    fn gt(&self, other: &Self) -> bool {
        let a = self.get();
        let b = other.get();

        a.gt(&b)
    }

    #[must_use]
    fn ge(&self, other: &Self) -> bool {
        let a = self.get();
        let b = other.get();

        a.ge(&b)
    }
}

#[derive(Debug)]
pub struct GcGuard<'a, T: Scan> {
    gc_ptr: &'a Gc<T>,
}

// TODO: Consider Send/Sync implementations for GcGuard

impl<'a, T: Scan> Drop for GcGuard<'a, T> {
    fn drop(&mut self) {
        let mut locked_collector = COLLECTOR.lock();
        if !locked_collector.validate_handle(&self.gc_ptr.backing_handle) {
            drop(locked_collector);
            panic!("Tried to drop a guard handle, but the internal state was corrupted (perhaps you're manipulating Gc<?> in a destructor?)");
        }

        locked_collector.dec_held_references();
    }
}

impl<'a, T: Scan> Deref for GcGuard<'a, T> {
    type Target = T;

    #[must_use]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.gc_ptr.direct_ptr }
    }
}

impl<'a, T: Scan> AsRef<T> for GcGuard<'a, T> {
    #[must_use]
    fn as_ref(&self) -> &T {
        self.deref()
    }
}

impl<'a, T: Scan> Borrow<T> for GcGuard<'a, T> {
    #[must_use]
    fn borrow(&self) -> &T {
        self.deref()
    }
}
