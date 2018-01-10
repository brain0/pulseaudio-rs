use std::mem;
use std::ops::{Deref, DerefMut};

pub enum ExplicitCleanup<T> {
    Exists(T),
    Cleaned
}

impl<T> ExplicitCleanup<T> {
    pub fn new(value: T) -> ExplicitCleanup<T> {
        ExplicitCleanup::Exists(value)
    }

    pub fn cleanup(this: &mut Self) -> Option<T> {
        match mem::replace(this, ExplicitCleanup::Cleaned) {
            ExplicitCleanup::Exists(value) => Some(value),
            ExplicitCleanup::Cleaned => None,
        }
    }
}

impl<T> Deref for ExplicitCleanup<T> {
    type Target = T;

    fn deref(&self) -> &T {
        match *self {
            ExplicitCleanup::Exists(ref value) => value,
            ExplicitCleanup::Cleaned => panic!("Trying to access a value that has already been cleaned up."),
        }
    }
}

impl<T> DerefMut for ExplicitCleanup<T> {
    fn deref_mut(&mut self) -> &mut T {
        match *self {
            ExplicitCleanup::Exists(ref mut value) => value,
            ExplicitCleanup::Cleaned => panic!("Trying to access a value that has already been cleaned up."),
        }
    }
}

impl<T: Clone> Clone for ExplicitCleanup<T> {
    fn clone(&self) -> ExplicitCleanup<T> {
        match *self {
            ExplicitCleanup::Exists(ref value) => ExplicitCleanup::Exists(value.clone()),
            ExplicitCleanup::Cleaned => ExplicitCleanup::Cleaned,
        }
    }
}
