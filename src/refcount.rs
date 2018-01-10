use std::ptr::null_mut;

pub unsafe trait RefCountable {
    fn incref(ptr: *mut Self);
    fn decref(ptr: *mut Self);
}

#[derive(Debug)]
pub struct RefCounted<T: RefCountable>(*mut T);

impl<T: RefCountable> RefCounted<T> {
    pub unsafe fn new(ptr: *mut T) -> RefCounted<T> {
        assert!(ptr != null_mut());
        RefCounted(ptr)
    }

    pub fn get(&self) -> *mut T {
        self.0
    }
}

impl<T: RefCountable> Clone for RefCounted<T> {
  fn clone(&self) -> Self {
      RefCountable::incref(self.0);
      RefCounted(self.0)
  }
}

impl<T: RefCountable> Drop for RefCounted<T> {
    fn drop(&mut self) {
        RefCountable::decref(self.0);
    }
}

macro_rules! pa_refcountable {
    ($t:ident, $incref:ident, $decref:ident) => {
        unsafe impl ::refcount::RefCountable for $t {
            fn incref(ptr: *mut $t) {
                unsafe { $incref(ptr) };
            }

            fn decref(ptr: *mut $t) {
                unsafe { $decref(ptr) };
            }
        }
    }
}
