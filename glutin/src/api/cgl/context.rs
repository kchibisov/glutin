use std::cell::Cell;
use std::ffi::{self, CStr};
use std::marker::PhantomData;
use std::rc::Rc;

use cgl::CGLSetParameter;
use cocoa::appkit::NSOpenGLContext;
use cocoa::base::{id, nil};
use core_foundation::base::TCFType;
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};
use core_foundation::string::CFString;
use objc::rc::autoreleasepool;
use objc::runtime::{BOOL, NO};

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
            let share_context = match context_attributes.shared_context.as_ref() {
                Some(RawContext::Cgl(share_context)) => share_context.cast(),
                _ => nil,
            };

            let config = config.clone();
            let raw = NSOpenGLContext::alloc(nil)
                .initWithFormat_shareContext_(config.inner.raw, share_context as *mut _);

            if config.inner.transrarency {
                let opacity = 0;
                super::check_error(CGLSetParameter(
                    raw.CGLContextObj().cast(),
                    cgl::kCGLCPSurfaceOpacity,
                    &opacity,
                ))?;
            }

            let inner = ContextInner { display: self.clone(), config, raw };
            let context = NotCurrentContext { inner, _nosync: PhantomData };

            Ok(context)
        }
    }
}

pub struct PossiblyCurrentContext {
    pub(crate) inner: ContextInner,
    // The context could be current only on the one thread.
    _nosendsync: PhantomData<Rc<()>>,
}

pub struct NotCurrentContext {
    pub(crate) inner: ContextInner,
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
        _surface_draw: &Self::Surface,
        _surface_read: &Self::Surface,
    ) -> Result<()> {
        Err(ErrorKind::NotSupported.into())
    }
}

impl PossiblyCurrentGlContext for PossiblyCurrentContext {
    type NotCurrentContext = NotCurrentContext;

    fn make_not_current(self) -> Result<Self::NotCurrentContext> {
        self.inner.make_not_current()?;
        Ok(NotCurrentContext::new(self.inner))
    }

    fn set_swap_interval(&self, interval: u16) {}

    fn is_current(&self) -> bool {
        autoreleasepool(|| unsafe {
            let current = NSOpenGLContext::currentContext(nil);
            if current != nil {
                let is_equal: BOOL = msg_send![current, isEqual: self.inner.raw];
                is_equal != NO
            } else {
                false
            }
        })
    }

    fn get_proc_address(&self, addr: &CStr) -> *const ffi::c_void {
        let symbol_name = CFString::new(addr.to_str().unwrap());
        let framework_name = CFString::new("com.apple.opengl");
        unsafe {
            let framework = CFBundleGetBundleWithIdentifier(framework_name.as_concrete_TypeRef());
            CFBundleGetFunctionPointerForName(framework, symbol_name.as_concrete_TypeRef()).cast()
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
        _surface_draw: &Self::Surface,
        _surface_read: &Self::Surface,
    ) -> Result<Self::PossiblyCurrentContext> {
        Err(ErrorKind::NotSupported.into())
    }
}

impl AsRawContext for PossiblyCurrentContext {
    fn raw_context(&self) -> RawContext {
        RawContext::Cgl(self.inner.raw.cast())
    }
}

impl AsRawContext for NotCurrentContext {
    fn raw_context(&self) -> RawContext {
        RawContext::Cgl(self.inner.raw.cast())
    }
}

pub(crate) struct ContextInner {
    display: Display,
    config: Config,
    pub(crate) raw: id,
}

impl ContextInner {
    fn make_current<T: SurfaceTypeTrait>(&self, surface: &Surface<T>) -> Result<()> {
        autoreleasepool(|| unsafe {
            self.raw.update();
            self.raw.makeCurrentContext();
            self.raw.setView_(surface.ns_view);
            Ok(())
        })
    }

    pub(crate) fn update(&self) {
        unsafe { self.raw.update() }
    }

    pub(crate) fn flush_buffer(&self) -> Result<()> {
        autoreleasepool(|| unsafe {
            self.raw.flushBuffer();
            Ok(())
        })
    }

    pub(crate) fn current_view(&self) -> id {
        unsafe { self.raw.view() }
    }

    fn make_not_current(&self) -> Result<()> {
        unsafe {
            self.raw.update();
            NSOpenGLContext::clearCurrentContext(nil);
            Ok(())
        }
    }
}

impl Drop for ContextInner {
    fn drop(&mut self) {
        unsafe {
            if self.raw != nil {
                let _: () = msg_send![self.raw, release];
            }
        }
    }
}
