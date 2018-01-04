use std::rc::Weak;
use std::ptr::null;

use super::TokioMainLoopApiImpl;

struct ExternalReferenceInner {
    parent: Weak<TokioMainLoopApiImpl>,
}

pub struct ExternalReference {
    inner: Box<ExternalReferenceInner>,
}

impl ExternalReference {
    pub fn new(parent: Weak<TokioMainLoopApiImpl>) -> ExternalReference {
        ExternalReference { inner: Box::new(ExternalReferenceInner{ parent }) }
    }

    pub fn as_ptr<T>(&self) -> *mut T {
        (&*self.inner) as *const _ as *mut T
    }

    pub unsafe fn run<T, R, F: FnOnce(&TokioMainLoopApiImpl) -> R>(ptr: *const T, f: F) -> R {
        assert!(ptr != null());
        let parent = (*(ptr as *const ExternalReferenceInner)).parent.upgrade().unwrap();
        f(&*parent)
    }
}
