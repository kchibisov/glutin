use std::ffi::{self, CStr};

use crate::dispatch_gl;
use crate::display::{Display, GetGlDisplay};
use crate::error::Result;
use crate::private::Sealed;
use crate::surface::{GlSurface, Surface, SurfaceTypeTrait};

#[cfg(cgl_backend)]
use crate::api::cgl::context::{
    NotCurrentContext as NotCurrentCglContext, PossiblyCurrentContext as PossiblyCurrentCglContext,
};
#[cfg(egl_backend)]
use crate::api::egl::context::{
    NotCurrentContext as NotCurrentEglContext, PossiblyCurrentContext as PossiblyCurrentEglContext,
};
#[cfg(glx_backend)]
use crate::api::glx::context::{
    NotCurrentContext as NotCurrentGlxContext, PossiblyCurrentContext as PossiblyCurrentGlxContext,
};
#[cfg(wgl_backend)]
use crate::api::wgl::context::{
    NotCurrentContext as NotCurrentWglContext, PossiblyCurrentContext as PossiblyCurrentWglContext,
};

/// Specifies the tolerance of the OpenGL [`Context`] to faults. If you accept
/// raw OpenGL commands and/or raw shader code from an untrusted source, you
/// should definitely care about this.
///
/// [`Context`]: crate::context::Context
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Robustness {
    /// Not everything is checked. Your application can crash if you do
    /// something wrong with your shaders.
    NotRobust,

    /// The driver doesn't check anything. This option is very dangerous.
    /// Please know what you're doing before using it. See the
    /// `GL_KHR_no_error` extension.
    ///
    /// Since this option is purely an optimization, no error will be returned
    /// if the backend doesn't support it. Instead it will automatically
    /// fall back to [`NotRobust`].
    ///
    /// [`NotRobust`]: crate::context::Robustness::NotRobust
    NoError,

    /// Everything is checked to avoid any crash. The driver will attempt to
    /// avoid any problem, but if a problem occurs the behavior is
    /// implementation-defined. You are just guaranteed not to get a crash.
    RobustNoResetNotification,

    /// Everything is checked to avoid any crash. If a problem occurs, the
    /// context will enter a "context lost" state. It must then be
    /// recreated.
    RobustLoseContextOnReset,
}

impl Default for Robustness {
    #[inline]
    fn default() -> Self {
        Robustness::NotRobust
    }
}

/// Describes the requested OpenGL [`Context`] profiles.
///
/// [`Context`]: crate::context::Context
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlProfile {
    /// Include all the immediate more functions and definitions.
    ///
    /// # Api specific
    ///
    /// Given that EGL does't support profiles it'll be used to represent OpenGL ES Api.
    Compatibility,
    /// Include all the future-compatible functions and definitions.
    ///
    /// # Api specific
    ///
    /// Given that EGL does't support profiles it'll be used to represent OpenGL Api.
    Core,
}

/// The behavior of the driver when you change the current context.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReleaseBehaviour {
    /// Doesn't do anything. Most notably doesn't flush. Not supported by all
    /// drivers.
    None,

    /// Flushes the context that was previously current as if `glFlush` was
    /// called. This is the default behaviour.
    Flush,
}

impl Default for ReleaseBehaviour {
    #[inline]
    fn default() -> Self {
        ReleaseBehaviour::Flush
    }
}

/// The attributes that are used to create a graphics context.
#[derive(Default)]
pub struct ContextAttributes {
    pub(crate) release_behavior: ReleaseBehaviour,

    pub(crate) debug: bool,

    pub(crate) robustness: Robustness,

    pub(crate) profile: Option<GlProfile>,

    pub(crate) shared_context: Option<RawContext>,
}

#[derive(Default)]
pub struct ContextAttributesBuilder {
    attributes: ContextAttributes,
}

impl ContextAttributesBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_debug(mut self, debug: bool) -> Self {
        self.attributes.debug = debug;
        self
    }

    pub fn with_sharing(mut self, context: &impl AsRawContext) -> Self {
        self.attributes.shared_context = Some(context.raw_context());
        self
    }

    pub fn with_robustness(mut self, robustness: Robustness) -> Self {
        self.attributes.robustness = robustness;
        self
    }

    pub fn with_profile(mut self, profile: GlProfile) -> Self {
        self.attributes.profile = Some(profile);
        self
    }

    pub fn with_release_behavior(mut self, release_behavior: ReleaseBehaviour) -> Self {
        self.attributes.release_behavior = release_behavior;
        self
    }

    pub fn build(self) -> ContextAttributes {
        self.attributes
    }
}

pub trait PossiblyCurrentGlContext: Sealed {
    type NotCurrentContext: NotCurrentGlContext;

    fn is_current(&self) -> bool;

    fn make_not_current(self) -> Result<Self::NotCurrentContext>;

    fn set_swap_interval(&self, interval: u16);

    fn update_after_resize(&self);

    fn get_proc_address(&self, addr: &CStr) -> *const ffi::c_void;
}

pub trait PossiblyCurrentContextGlSurfaceAccessor<T: SurfaceTypeTrait>: Sealed {
    type Surface: GlSurface<T>;

    fn make_current(&self, surface: &Self::Surface) -> Result<()>;

    fn make_current_draw_read(
        &self,
        surface_draw: &Self::Surface,
        surface_read: &Self::Surface,
    ) -> Result<()>;
}

pub trait NotCurrentGlContext: Sealed {
    type PossiblyCurrentContext: PossiblyCurrentGlContext;

    fn treat_as_current(self) -> Self::PossiblyCurrentContext;
}

pub trait NotCurrentGlContextSurfaceAccessor<T: SurfaceTypeTrait>: Sealed {
    type Surface: GlSurface<T>;
    type PossiblyCurrentContext: PossiblyCurrentGlContext;

    fn make_current(self, surface: &Self::Surface) -> Result<Self::PossiblyCurrentContext>;

    fn make_current_draw_read(
        self,
        surface_draw: &Self::Surface,
        surface_read: &Self::Surface,
    ) -> Result<Self::PossiblyCurrentContext>;
}

pub enum PossiblyCurrentContext {
    #[cfg(egl_backend)]
    Egl(PossiblyCurrentEglContext),

    #[cfg(glx_backend)]
    Glx(PossiblyCurrentGlxContext),

    #[cfg(wgl_backend)]
    Wgl(PossiblyCurrentWglContext),

    #[cfg(cgl_backend)]
    Cgl(PossiblyCurrentCglContext),
}

impl PossiblyCurrentGlContext for PossiblyCurrentContext {
    type NotCurrentContext = NotCurrentContext;

    fn is_current(&self) -> bool {
        dispatch_gl!(self; Self(context) => context.is_current())
    }

    fn make_not_current(self) -> Result<Self::NotCurrentContext> {
        Ok(dispatch_gl!(self; Self(context) => context.make_not_current()?; as NotCurrentContext))
    }

    fn set_swap_interval(&self, interval: u16) {
        dispatch_gl!(self; Self(context) => context.set_swap_interval(interval))
    }

    fn update_after_resize(&self) {
        dispatch_gl!(self; Self(context) => context.update_after_resize())
    }

    fn get_proc_address(&self, addr: &CStr) -> *const ffi::c_void {
        dispatch_gl!(self; Self(context) => context.get_proc_address(addr))
    }
}

impl Sealed for PossiblyCurrentContext {}
impl Sealed for NotCurrentContext {}

impl<T: SurfaceTypeTrait> PossiblyCurrentContextGlSurfaceAccessor<T> for PossiblyCurrentContext {
    type Surface = Surface<T>;

    fn make_current(&self, surface: &Self::Surface) -> Result<()> {
        match (self, surface) {
            #[cfg(egl_backend)]
            (Self::Egl(context), Surface::Egl(surface)) => context.make_current(surface),
            #[cfg(glx_backend)]
            (Self::Glx(context), Surface::Glx(surface)) => context.make_current(surface),
            #[cfg(wgl_backend)]
            (Self::Wgl(context), Surface::Wgl(surface)) => context.make_current(surface),
            #[cfg(cgl_backend)]
            (Self::Cgl(context), Surface::Cgl(surface)) => context.make_current(surface),
            _ => unreachable!(),
        }
    }

    fn make_current_draw_read(
        &self,
        surface_draw: &Self::Surface,
        surface_read: &Self::Surface,
    ) -> Result<()> {
        match (self, surface_draw, surface_read) {
            #[cfg(egl_backend)]
            (Self::Egl(context), Surface::Egl(draw), Surface::Egl(read)) => {
                context.make_current_draw_read(draw, read)
            }
            #[cfg(glx_backend)]
            (Self::Glx(context), Surface::Glx(draw), Surface::Glx(read)) => {
                context.make_current_draw_read(draw, read)
            }
            #[cfg(wgl_backend)]
            (Self::Wgl(context), Surface::Wgl(draw), Surface::Wgl(read)) => {
                context.make_current_draw_read(draw, read)
            }
            #[cfg(cgl_backend)]
            (Self::Cgl(context), Surface::Cgl(draw), Surface::Cgl(read)) => {
                context.make_current_draw_read(draw, read)
            }
            _ => unreachable!(),
        }
    }
}

impl GetGlDisplay for PossiblyCurrentContext {
    type Target = Display;
    fn display(&self) -> Self::Target {
        dispatch_gl!(self; Self(context) => context.display(); as Display)
    }
}

pub enum NotCurrentContext {
    #[cfg(egl_backend)]
    Egl(NotCurrentEglContext),

    #[cfg(glx_backend)]
    Glx(NotCurrentGlxContext),

    #[cfg(wgl_backend)]
    Wgl(NotCurrentWglContext),

    #[cfg(cgl_backend)]
    Cgl(NotCurrentCglContext),
}

impl<T: SurfaceTypeTrait> NotCurrentGlContextSurfaceAccessor<T> for NotCurrentContext {
    type PossiblyCurrentContext = PossiblyCurrentContext;
    type Surface = Surface<T>;

    fn make_current(self, surface: &Self::Surface) -> Result<Self::PossiblyCurrentContext> {
        match (self, surface) {
            #[cfg(egl_backend)]
            (Self::Egl(context), Surface::Egl(surface)) => {
                Ok(PossiblyCurrentContext::Egl(context.make_current(surface)?))
            }
            #[cfg(glx_backend)]
            (Self::Glx(context), Surface::Glx(surface)) => {
                Ok(PossiblyCurrentContext::Glx(context.make_current(surface)?))
            }
            #[cfg(wgl_backend)]
            (Self::Wgl(context), Surface::Wgl(surface)) => {
                Ok(PossiblyCurrentContext::Wgl(context.make_current(surface)?))
            }
            #[cfg(cgl_backend)]
            (Self::Cgl(context), Surface::Cgl(surface)) => {
                Ok(PossiblyCurrentContext::Cgl(context.make_current(surface)?))
            }
            _ => unreachable!(),
        }
    }

    fn make_current_draw_read(
        self,
        surface_draw: &Self::Surface,
        surface_read: &Self::Surface,
    ) -> Result<Self::PossiblyCurrentContext> {
        match (self, surface_draw, surface_read) {
            #[cfg(egl_backend)]
            (Self::Egl(context), Surface::Egl(draw), Surface::Egl(read)) => {
                Ok(PossiblyCurrentContext::Egl(context.make_current_draw_read(draw, read)?))
            }
            #[cfg(glx_backend)]
            (Self::Glx(context), Surface::Glx(draw), Surface::Glx(read)) => {
                Ok(PossiblyCurrentContext::Glx(context.make_current_draw_read(draw, read)?))
            }
            #[cfg(wgl_backend)]
            (Self::Wgl(context), Surface::Wgl(draw), Surface::Wgl(read)) => {
                Ok(PossiblyCurrentContext::Wgl(context.make_current_draw_read(draw, read)?))
            }
            #[cfg(cgl_backend)]
            (Self::Cgl(context), Surface::Cgl(draw), Surface::Cgl(read)) => {
                Ok(PossiblyCurrentContext::Cgl(context.make_current_draw_read(draw, read)?))
            }
            _ => unreachable!(),
        }
    }
}

impl NotCurrentGlContext for NotCurrentContext {
    type PossiblyCurrentContext = PossiblyCurrentContext;

    fn treat_as_current(self) -> Self::PossiblyCurrentContext {
        dispatch_gl!(self; Self(context) => context.treat_as_current(); as PossiblyCurrentContext)
    }
}

impl GetGlDisplay for NotCurrentContext {
    type Target = Display;
    fn display(&self) -> Self::Target {
        dispatch_gl!(self; Self(context) => context.display(); as Display)
    }
}

/// Raw context.
pub enum RawContext {
    /// Raw EGL context.
    #[cfg(egl_backend)]
    Egl(*const ffi::c_void),

    /// Raw GLX context.
    #[cfg(glx_backend)]
    Glx(*const ffi::c_void),

    /// TODO.
    #[cfg(wgl_backend)]
    Wgl(*const ffi::c_void),

    /// TODO.
    #[cfg(cgl_backend)]
    Cgl(*const ffi::c_void),
}

pub trait AsRawContext {
    fn raw_context(&self) -> RawContext;
}

impl AsRawContext for PossiblyCurrentContext {
    fn raw_context(&self) -> RawContext {
        dispatch_gl!(self; Self(context) => context.raw_context())
    }
}

impl AsRawContext for NotCurrentContext {
    fn raw_context(&self) -> RawContext {
        dispatch_gl!(self; Self(context) => context.raw_context())
    }
}
