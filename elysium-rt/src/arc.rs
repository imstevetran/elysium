// Automatic Reference Counting runtime for Elysium
// Manages retain/release for heap-allocated objects.

use std::cell::Cell;
use std::marker::PhantomData;
use std::ptr::NonNull;

/// A reference-counted pointer for Elysium objects.
pub struct Ref<T> {
    ptr: Option<NonNull<RefInner<T>>>,
    _marker: PhantomData<T>,
}

struct RefInner<T> {
    ref_count: Cell<usize>,
    weak_count: Cell<usize>,
    value: T,
}

impl<T> Ref<T> {
    pub fn new(value: T) -> Self {
        let inner = Box::new(RefInner {
            ref_count: Cell::new(1),
            weak_count: Cell::new(0),
            value,
        });
        Ref {
            ptr: Some(NonNull::new(Box::into_raw(inner)).unwrap()),
            _marker: PhantomData,
        }
    }

    pub fn retain(this: &Self) {
        if let Some(ptr) = this.ptr {
            let inner = unsafe { ptr.as_ref() };
            let count = inner.ref_count.get();
            inner.ref_count.set(count + 1);
        }
    }

    pub fn release(this: &Self) {
        if let Some(ptr) = this.ptr {
            let inner = unsafe { ptr.as_ref() };
            let count = inner.ref_count.get();
            if count <= 1 {
                // Deallocate
                unsafe {
                    let _ = Box::from_raw(ptr.as_ptr());
                }
            } else {
                inner.ref_count.set(count - 1);
            }
        }
    }

    pub fn borrow(&self) -> Option<&T> {
        self.ptr.map(|ptr| unsafe { &ptr.as_ref().value })
    }

    pub fn borrow_mut(&mut self) -> Option<&mut T> {
        self.ptr
            .as_mut()
            .map(|ptr| unsafe { &mut ptr.as_ptr().as_mut().unwrap().value })
    }

    pub fn is_valid(&self) -> bool {
        self.ptr.is_some()
    }
}

impl<T> Clone for Ref<T> {
    fn clone(&self) -> Self {
        Self::retain(self);
        Ref {
            ptr: self.ptr,
            _marker: PhantomData,
        }
    }
}

impl<T> Drop for Ref<T> {
    fn drop(&mut self) {
        if self.ptr.is_some() {
            Self::release(self);
        }
    }
}

/// A weak reference that does not keep the object alive.
pub struct Weak<T> {
    ptr: Option<NonNull<RefInner<T>>>,
    _marker: PhantomData<T>,
}

impl<T> Weak<T> {
    pub fn new(from: &Ref<T>) -> Self {
        if let Some(ptr) = from.ptr {
            let inner = unsafe { ptr.as_ref() };
            let count = inner.weak_count.get();
            inner.weak_count.set(count + 1);
            Weak {
                ptr: Some(ptr),
                _marker: PhantomData,
            }
        } else {
            Weak {
                ptr: None,
                _marker: PhantomData,
            }
        }
    }

    pub fn upgrade(&self) -> Option<Ref<T>> {
        self.ptr.and_then(|ptr| {
            let inner = unsafe { ptr.as_ref() };
            if inner.ref_count.get() > 0 {
                let count = inner.ref_count.get();
                inner.ref_count.set(count + 1);
                Some(Ref {
                    ptr: self.ptr,
                    _marker: PhantomData,
                })
            } else {
                None
            }
        })
    }
}

impl<T> Drop for Weak<T> {
    fn drop(&mut self) {
        if let Some(ptr) = self.ptr {
            let inner = unsafe { ptr.as_ref() };
            let count = inner.weak_count.get();
            inner.weak_count.set(count - 1);
            if inner.ref_count.get() == 0 && inner.weak_count.get() == 0 {
                unsafe {
                    let _ = Box::from_raw(ptr.as_ptr());
                }
            }
        }
    }
}

/// An unowned reference (no retain/release, assumes object outlives reference).
pub struct Unowned<T> {
    ptr: *const T,
    _marker: PhantomData<T>,
}

impl<T> Unowned<T> {
    pub fn new(from: &Ref<T>) -> Self {
        let ptr = from
            .ptr
            .map(|p| {
                let inner = unsafe { p.as_ref() };
                &inner.value as *const T
            })
            .unwrap_or(std::ptr::null());
        Unowned {
            ptr,
            _marker: PhantomData,
        }
    }

    pub fn get(&self) -> Option<&T> {
        if self.ptr.is_null() {
            None
        } else {
            Some(unsafe { &*self.ptr })
        }
    }
}
