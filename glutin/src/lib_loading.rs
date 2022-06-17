//! Library loading routines.

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use libloading::Library;

#[derive(Clone)]
pub struct SymWrapper<T> {
    sym: T,
    _lib: Arc<Library>,
}

pub trait SymLoading {
    fn load_with(lib: &Library) -> Self;
}

impl<T: SymLoading> SymWrapper<T> {
    pub fn new(libs: &[&str]) -> Result<Self, ()> {
        unsafe {
            for lib in libs {
                if let Ok(lib) = Library::new(lib) {
                    return Ok(SymWrapper { sym: T::load_with(&lib), _lib: Arc::new(lib) });
                }
            }
        }

        Err(())
    }
}

impl<T> Deref for SymWrapper<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.sym
    }
}

impl<T> DerefMut for SymWrapper<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sym
    }
}
