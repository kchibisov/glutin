use std::cell::Cell;
use std::ffi::{self, CStr};
use std::marker::PhantomData;
use std::rc::Rc;

use glutin_egl_sys::egl::types::EGLint;
use glutin_egl_sys::{egl, EGLContext};

use crate::config::{Api, GetGlConfig};
use crate::context::{AsRawContext, ContextAttributes, GlProfile, RawContext, Robustness};
use crate::display::GetGlDisplay;
use crate::error::{Error, ErrorKind, Result};
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
        let mut attrs = Vec::<EGLint>::new();

        // Get the api supported by the config.
        let api = config.api();

        let supports_opengl = !(self.inner.version.major <= 1 && self.inner.version.minor <= 3);
        let supports_gles = !(self.inner.version.major <= 1 && self.inner.version.minor <= 1);

        let api = match context_attributes.profile {
            Some(GlProfile::Core) if supports_opengl => egl::OPENGL_API,
            Some(GlProfile::Compatibility) if supports_gles => egl::OPENGL_ES_API,
            None if api.contains(Api::OPENGL) && supports_opengl => egl::OPENGL_API,
            None if supports_gles => egl::OPENGL_ES_API,
            _ => return Err(ErrorKind::NotSupported.into()),
        };

        let is_one_five = !(self.inner.version.major <= 1 && self.inner.version.minor <= 4);
        if is_one_five || self.inner.client_extensions.contains("EGL_KHR_create_context") {
            let mut flags = 0;

            let has_robustsess = is_one_five
                || self.inner.client_extensions.contains("EGL_EXT_create_context_robustness");
            let has_no_error =
                self.inner.client_extensions.contains("EGL_KHR_create_context_no_error");

            match context_attributes.robustness {
                Robustness::NotRobust => (),
                Robustness::NoError if has_no_error => {
                    attrs.push(egl::CONTEXT_OPENGL_NO_ERROR_KHR as EGLint);
                    attrs.push(egl::TRUE as EGLint);
                }
                Robustness::RobustLoseContextOnReset if has_robustsess => {
                    attrs.push(egl::CONTEXT_OPENGL_RESET_NOTIFICATION_STRATEGY as EGLint);
                    attrs.push(egl::LOSE_CONTEXT_ON_RESET as EGLint);
                    flags |= egl::CONTEXT_OPENGL_ROBUST_ACCESS;
                }
                Robustness::RobustNoResetNotification if has_robustsess => {
                    attrs.push(egl::CONTEXT_OPENGL_RESET_NOTIFICATION_STRATEGY as EGLint);
                    attrs.push(egl::NO_RESET_NOTIFICATION as EGLint);
                    flags |= egl::CONTEXT_OPENGL_ROBUST_ACCESS;
                }
                _ => {
                    return Err(Error::new(
                        None,
                        Some("Context robustness is not supported.".into()),
                        ErrorKind::NotSupported,
                    ))
                }
            }

            if context_attributes.debug && is_one_five && !has_no_error {
                attrs.push(egl::CONTEXT_OPENGL_DEBUG as EGLint);
                attrs.push(egl::TRUE as EGLint);
            }

            if flags != 0 {
                attrs.push(egl::CONTEXT_FLAGS_KHR as EGLint);
                attrs.push(flags as EGLint);
            }
        }

        attrs.push(egl::NONE as EGLint);

        let shared_context =
            if let Some(shared_context) = context_attributes.shared_context.as_ref() {
                match shared_context {
                    RawContext::Egl(shared_context) => *shared_context,
                    _ => unreachable!(),
                }
            } else {
                egl::NO_CONTEXT
            };

        // Bind the api.
        unsafe {
            if self.inner.egl.BindAPI(api) == egl::FALSE {
                return Err(super::check_error().err().unwrap());
            }
        }

        unsafe {
            let config = config.clone();
            let context = self.inner.egl.CreateContext(
                self.inner.raw,
                config.inner.raw,
                shared_context,
                attrs.as_ptr(),
            );

            if context == egl::NO_CONTEXT {
                return Err(super::check_error().err().unwrap());
            }

            let inner = ContextInner { display: self.clone(), config, raw: context };
            Ok(NotCurrentContext::new(inner))
        }
    }
}

pub struct PossiblyCurrentContext {
    inner: ContextInner,
    // The context could be current only on the one thread, a.
    _nosendsync: PhantomData<Rc<()>>,
}

pub struct NotCurrentContext {
    inner: ContextInner,
    // Only non-current context could be send between threads safely.
    _nosync: PhantomData<Cell<()>>,
}

impl NotCurrentContext {
    fn new(inner: ContextInner) -> Self {
        Self { inner, _nosync: PhantomData }
    }
}

impl Sealed for PossiblyCurrentContext {}
impl Sealed for NotCurrentContext {}

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

    fn is_current(&self) -> bool {
        unsafe { self.inner.display.inner.egl.GetCurrentContext() == self.inner.raw }
    }

    fn set_swap_interval(&self, interval: u16) {}

    fn get_proc_address(&self, addr: &CStr) -> *const ffi::c_void {
        unsafe { self.inner.display.inner.egl.GetProcAddress(addr.as_ptr()) as *const _ }
    }
}

impl NotCurrentGlContext for NotCurrentContext {
    type PossiblyCurrentContext = PossiblyCurrentContext;

    fn treat_as_current(self) -> Self::PossiblyCurrentContext {
        PossiblyCurrentContext { inner: self.inner, _nosendsync: PhantomData }
    }
}

impl<T: SurfaceTypeTrait> NotCurrentGlContextSurfaceAccessor<T> for NotCurrentContext {
    type Surface = Surface<T>;
    type PossiblyCurrentContext = PossiblyCurrentContext;

    fn make_current(self, surface: &Surface<T>) -> Result<PossiblyCurrentContext> {
        self.inner.make_current_draw_read(surface, surface)?;
        Ok(PossiblyCurrentContext { inner: self.inner, _nosendsync: PhantomData })
    }

    fn make_current_draw_read(
        self,
        surface_draw: &Surface<T>,
        surface_read: &Surface<T>,
    ) -> Result<PossiblyCurrentContext> {
        self.inner.make_current_draw_read(surface_draw, surface_read)?;
        Ok(PossiblyCurrentContext { inner: self.inner, _nosendsync: PhantomData })
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

impl AsRawContext for PossiblyCurrentContext {
    fn raw_context(&self) -> RawContext {
        RawContext::Egl(self.inner.raw)
    }
}

impl AsRawContext for NotCurrentContext {
    fn raw_context(&self) -> RawContext {
        RawContext::Egl(self.inner.raw)
    }
}

struct ContextInner {
    display: Display,
    config: Config,
    raw: EGLContext,
}

impl ContextInner {
    fn make_current_draw_read<T: SurfaceTypeTrait>(
        &self,
        surface_draw: &Surface<T>,
        surface_read: &Surface<T>,
    ) -> Result<()> {
        unsafe {
            let draw = surface_draw.raw;
            let read = surface_read.raw;
            if self.display.inner.egl.MakeCurrent(self.display.inner.raw, draw, read, self.raw)
                == egl::FALSE
            {
                super::check_error()
            } else {
                Ok(())
            }
        }
    }

    fn make_not_current(&self) -> Result<()> {
        unsafe {
            if self.display.inner.egl.MakeCurrent(
                self.display.inner.raw,
                egl::NO_SURFACE,
                egl::NO_SURFACE,
                egl::NO_CONTEXT,
            ) == egl::FALSE
            {
                super::check_error()
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
            self.display.inner.egl.DestroyContext(self.display.inner.raw, self.raw);
        }
    }
}
