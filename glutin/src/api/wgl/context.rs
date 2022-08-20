use std::cell::Cell;
use std::ffi::{self, CStr};
use std::marker::PhantomData;
use std::rc::Rc;

use glutin_wgl_sys::wgl;
use glutin_wgl_sys::wgl::types::HGLRC;
use raw_window_handle::RawWindowHandle;
use windows_sys::Win32::Graphics::Gdi::{self as gdi};
use windows_sys::Win32::System::LibraryLoader as dll_loader;

use crate::config::GetGlConfig;
use crate::context::{AsRawContext, ContextAttributes, RawContext};
use crate::display::GetGlDisplay;
use crate::error::{ErrorKind, Result};
use crate::prelude::*;
use crate::private::Sealed;
use crate::surface::SurfaceTypeTrait;

use super::config::Config;
use super::display::Display;
use super::surface::Surface;

impl Display {
    pub(crate) fn create_context(
        &self,
        config: &Config,
        context_attributes: &ContextAttributes,
    ) -> Result<NotCurrentContext> {
        unsafe {
            let hdc = match context_attributes.raw_window_handle.as_ref() {
                handle @ Some(RawWindowHandle::Win32(window)) => {
                    let _ = config.apply_on_native_window(handle.unwrap());
                    gdi::GetDC(window.hwnd as _)
                }
                _ => config.inner.hdc,
            };

            let raw = wgl::CreateContext(hdc as *const _);
            if raw.is_null() {
                return Err(ErrorKind::BadConfig.into());
            }
            let inner = ContextInner { display: self.clone(), config: config.clone(), raw };
            Ok(NotCurrentContext { inner, _nosync: PhantomData })
        }
    }
}

pub struct PossiblyCurrentContext {
    inner: ContextInner,
    // The context could be current only on the one thread.
    _nosendsync: PhantomData<Rc<()>>,
}

pub struct NotCurrentContext {
    inner: ContextInner,
    // Only non-current context could be send between threads safely.
    _nosync: PhantomData<Cell<()>>,
}

impl Sealed for PossiblyCurrentContext {}
impl Sealed for NotCurrentContext {}

impl NotCurrentContext {
    fn new(inner: ContextInner) -> Self {
        Self { inner, _nosync: PhantomData }
    }
}

impl GetGlDisplay for NotCurrentContext {
    type Target = Display;
    fn display(&self) -> Self::Target {
        self.inner.display.clone()
    }
}

impl GetGlDisplay for PossiblyCurrentContext {
    type Target = Display;
    fn display(&self) -> Self::Target {
        self.inner.display.clone()
    }
}

impl GetGlConfig for NotCurrentContext {
    type Target = Config;
    fn config(&self) -> Self::Target {
        self.inner.config.clone()
    }
}

impl GetGlConfig for PossiblyCurrentContext {
    type Target = Config;
    fn config(&self) -> Self::Target {
        self.inner.config.clone()
    }
}

impl<T: SurfaceTypeTrait> PossiblyCurrentContextGlSurfaceAccessor<T> for PossiblyCurrentContext {
    type Surface = Surface<T>;

    fn make_current(&self, surface: &Self::Surface) -> Result<()> {
        self.inner.make_current(surface)
    }

    fn make_current_draw_read(
        &self,
        surface_draw: &Self::Surface,
        surface_read: &Self::Surface,
    ) -> Result<()> {
        self.inner.make_current_draw_read(surface_draw, surface_read)
    }
}

impl PossiblyCurrentGlContext for PossiblyCurrentContext {
    type NotCurrentContext = NotCurrentContext;

    fn make_not_current(self) -> Result<Self::NotCurrentContext> {
        unsafe {
            // TODO error
            if self.is_current() {
                let hdc = wgl::GetCurrentDC();
                wgl::MakeCurrent(hdc, std::ptr::null());
            }

            Ok(NotCurrentContext::new(self.inner))
        }
    }

    fn set_swap_interval(&self, interval: u16) {}

    fn is_current(&self) -> bool {
        unsafe { wgl::GetCurrentContext() == self.inner.raw }
    }

    fn get_proc_address(&self, addr: &CStr) -> *const ffi::c_void {
        unsafe {
            let addr = addr.as_ptr();
            let fn_ptr = wgl::GetProcAddress(addr);
            if !fn_ptr.is_null() {
                fn_ptr.cast()
            } else {
                dll_loader::GetProcAddress(self.inner.display.inner.lib_opengl32, addr.cast())
                    .map_or(std::ptr::null(), |fn_ptr| fn_ptr as *const _)
            }
        }
    }
}

impl NotCurrentGlContext for NotCurrentContext {
    type PossiblyCurrentContext = PossiblyCurrentContext;

    fn treat_as_current(self) -> PossiblyCurrentContext {
        PossiblyCurrentContext { inner: self.inner, _nosendsync: PhantomData }
    }
}

impl<T: SurfaceTypeTrait> NotCurrentGlContextSurfaceAccessor<T> for NotCurrentContext {
    type Surface = Surface<T>;
    type PossiblyCurrentContext = PossiblyCurrentContext;

    fn make_current(self, surface: &Self::Surface) -> Result<Self::PossiblyCurrentContext> {
        self.inner.make_current(surface)?;
        Ok(PossiblyCurrentContext { inner: self.inner, _nosendsync: PhantomData })
    }

    fn make_current_draw_read(
        self,
        surface_draw: &Self::Surface,
        surface_read: &Self::Surface,
    ) -> Result<Self::PossiblyCurrentContext> {
        self.inner.make_current_draw_read(surface_draw, surface_read)?;
        Ok(PossiblyCurrentContext { inner: self.inner, _nosendsync: PhantomData })
    }
}

impl AsRawContext for PossiblyCurrentContext {
    fn raw_context(&self) -> RawContext {
        RawContext::Wgl(self.inner.raw)
    }
}

impl AsRawContext for NotCurrentContext {
    fn raw_context(&self) -> RawContext {
        RawContext::Wgl(self.inner.raw)
    }
}

struct ContextInner {
    display: Display,
    config: Config,
    raw: HGLRC,
}

impl ContextInner {
    fn make_current_draw_read<T: SurfaceTypeTrait>(
        &self,
        _surface_draw: &Surface<T>,
        _surface_read: &Surface<T>,
    ) -> Result<()> {
        Err(ErrorKind::NotSupported.into())
    }

    fn make_current<T: SurfaceTypeTrait>(&self, surface: &Surface<T>) -> Result<()> {
        unsafe {
            let hdc = gdi::GetDC(surface.hwnd);
            wgl::MakeCurrent(hdc as _, self.raw.cast());
            Ok(())
        }
    }
}

impl Drop for ContextInner {
    fn drop(&mut self) {
        unsafe {
            wgl::DeleteContext(self.raw);
        }
    }
}
