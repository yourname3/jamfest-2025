// Integration with the PonieScript GC.

use std::{ops::Deref, ptr, sync::atomic::{AtomicPtr, AtomicU64, Ordering}};

pub trait Gc {
    const TYPE_ID: u64;
}

#[macro_export]
macro_rules! gc {
    ($typ:ty, $typ_id:expr) => {
        impl crate::gc::Gc for $typ {
            const TYPE_ID: u64 = $typ_id;
        }
    };
}

#[repr(C)]
pub struct GcValue<T> {
    header: AtomicU64,
    inner: T,
}

// Garbage-collected pointer.
pub struct Gp<T: Gc> {
    // TODO: Is there any way to make this a NonNull?
    ptr: AtomicPtr<GcValue<T>>
}

pub struct GpMaybe<T: Gc> {
    ptr: AtomicPtr<GcValue<T>>
}

impl<T: Gc> Gp<T> {
    pub fn new(t: T) -> Gp<T> {
        // TODO: We want to actually do the allocation through the GC. For now,
        // let's just leak a box.
        let value = Box::new(GcValue {
            header: AtomicU64::new(T::TYPE_ID),
            inner: t
        });

        let as_static_ref = Box::leak(value);

        Gp {
            ptr: AtomicPtr::new(as_static_ref as *mut GcValue<T>)
        }
    }

    pub fn set(&self, other: &Gp<T>) {
        // TODO:
        // Do we want this type to do a write barrier, or do we only want
        // a specific "field ptr" type to do a write barrier?
        self.ptr.store(other.ptr.load(Ordering::Relaxed), Ordering::Relaxed);
    }

    pub fn get_gc_value_ptr(&self) -> &GcValue<T> {
        let ptr = self.ptr.load(Ordering::Relaxed);

        // SAFETY:
        // We should not be allowed to allocate a non-null garbage collected
        // object. If this pointer is pointing to an invalid location, that is
        // a garbage collection bug.
        unsafe { &*ptr }
    }

    pub fn has_same_id(&self, other: &Gp<T>) -> bool {
        self.ptr.load(Ordering::Relaxed) == other.ptr.load(Ordering::Relaxed)
    }
}

impl<T: Gc> GpMaybe<T> {
    pub fn none() -> Self {
        GpMaybe { ptr: ptr::null_mut::<GcValue<T>>().into() }
    }

    #[inline(always)]
    pub fn set(&self, other: Option<&Gp<T>>) {
        match other {
            Some(other) => self.ptr.store(other.ptr.load(Ordering::Relaxed), Ordering::Relaxed),
            None => self.ptr.store(ptr::null_mut::<GcValue<T>>(), Ordering::Relaxed)
        }   
    }

    #[inline(always)]
    pub fn get(&self) -> Option<Gp<T>> {
        let ptr = self.ptr.load(Ordering::Relaxed);
        if ptr.is_null() {
            return None;
        }
        Some(Gp { ptr: ptr.into() })
    }
}

impl<T: Gc> Deref for Gp<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.get_gc_value_ptr().inner
    }
}

impl<T: Gc> AsRef<T> for Gp<T> {
    fn as_ref(&self) -> &T {
        &self.get_gc_value_ptr().inner
    }
}

impl<T: Gc> Clone for Gp<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self { ptr: AtomicPtr::new(self.ptr.load(Ordering::Relaxed)) }
    }
}

impl<T: Gc> Clone for GpMaybe<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self { ptr: AtomicPtr::new(self.ptr.load(Ordering::Relaxed)) }
    }
}