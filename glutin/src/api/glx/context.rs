use std::cell::Cell;
use std::ffi::{self, CStr};
use std::marker::PhantomData;
use std::os::raw::c_int;
use std::rc::Rc;

use glutin_glx_sys::glx;
use glutin_glx_sys::glx::types::GLXContext;
use glutin_glx_sys::glx_extra;

use crate::context::{AsRawContext, ContextAttributes, RawContext};

use crate::config::GetGlConfig;
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
        let mut attrs = Vec::<c_int>::with_capacity(4);
        attrs.push(glx::NONE as c_int);

        let shared_context =
            if let Some(shared_context) = context_attributes.shared_context.as_ref() {
                match shared_context {
                    RawContext::Glx(shared_context) => *shared_context,
                    _ => return Err(ErrorKind::NotSupported.into()),
                }
            } else {
                std::ptr::null()
            };

        unsafe {
            let render_type =
                if config.float_pixels() { glx_extra::RGBA_FLOAT_TYPE_ARB } else { glx::RGBA_TYPE };

            let config = config.clone();
            let context = self.inner.glx.CreateNewContext(
                self.inner.raw.cast(),
                config.inner.raw,
                render_type as c_int,
                shared_context,
                // Direct context.
                1,
            );

            super::last_glx_error(self.inner.raw)?;

            let inner = ContextInner { display: self.clone(), config, context };

            Ok(NotCurrentContext::new(inner))
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
        self.inner.make_current_draw_read(surface, surface)
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
        self.inner.make_not_current()?;
        Ok(NotCurrentContext::new(self.inner))
    }

    fn update_after_resize(&self) {
        self.inner.update_after_resize()
    }

    fn set_swap_interval(&self, interval: u16) {}

    fn is_current(&self) -> bool {
        unsafe { self.inner.display.inner.glx.GetCurrentContext() == self.inner.context }
    }

    fn get_proc_address(&self, addr: &CStr) -> *const ffi::c_void {
        unsafe {
            self.inner.display.inner.glx.GetProcAddress(addr.as_ptr() as *const _) as *const _
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
        self.inner.make_current_draw_read(surface, surface)?;
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
        RawContext::Glx(self.inner.context)
    }
}

impl AsRawContext for NotCurrentContext {
    fn raw_context(&self) -> RawContext {
        RawContext::Glx(self.inner.context)
    }
}

struct ContextInner {
    display: Display,
    config: Config,
    context: GLXContext,
}

impl ContextInner {
    fn make_current_draw_read<T: SurfaceTypeTrait>(
        &self,
        surface_draw: &Surface<T>,
        surface_read: &Surface<T>,
    ) -> Result<()> {
        unsafe {
            if self.display.inner.glx.MakeContextCurrent(
                self.display.inner.raw.cast(),
                surface_draw.raw,
                surface_read.raw,
                self.context,
            ) == 0
            {
                super::last_glx_error(self.display.inner.raw)
            } else {
                Ok(())
            }
        }
    }

    fn make_not_current(&self) -> Result<()> {
        unsafe {
            if self.display.inner.glx.MakeContextCurrent(
                self.display.inner.raw.cast(),
                0,
                0,
                std::ptr::null(),
            ) == 0
            {
                super::last_glx_error(self.display.inner.raw)
            } else {
                Ok(())
            }
        }
    }

    fn update_after_resize(&self) {
        // This line is intentionally left blank.
    }
}

impl Drop for ContextInner {
    fn drop(&mut self) {
        unsafe {
            self.display.inner.glx.DestroyContext(self.display.inner.raw.cast(), self.context);
        }
    }
}
