//! The GL platform display creation and picking.
#![allow(unreachable_patterns)]

use raw_window_handle::RawDisplayHandle;

use crate::config::{Config, ConfigTemplate, GlConfig};
use crate::context::{ContextAttributes, NotCurrentContext, NotCurrentGlContext};
use crate::dispatch_gl;
use crate::error::Result;
use crate::private::Sealed;
use crate::surface::{
    GlSurface, PbufferSurface, PixmapSurface, Surface, SurfaceAttributes, WindowSurface,
};

#[cfg(cgl_backend)]
use crate::api::cgl::display::Display as CglDisplay;
#[cfg(egl_backend)]
use crate::api::egl::display::Display as EglDisplay;
#[cfg(glx_backend)]
use crate::api::glx::display::Display as GlxDisplay;
#[cfg(glx_backend)]
use crate::api::glx::XlibErrorHookRegistrator;
#[cfg(wgl_backend)]
use crate::api::wgl::display::Display as WglDisplay;

/// Preference of the display that should be used.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DisplayApiPreference {
    /// Prefer EGL.
    #[cfg(egl_backend)]
    Egl,
    /// Prefer GLX.
    #[cfg(glx_backend)]
    Glx,
    /// Prefer WGL.
    #[cfg(wgl_backend)]
    Wgl,
    /// Prefer CGL.
    #[cfg(cgl_backend)]
    Cgl,

    /// Prefer EGL and fallback to GLX.
    #[cfg(all(egl_backend, glx_backend))]
    EglThenGlx,
    /// Prefer GLX and fallback to EGL.
    #[cfg(all(egl_backend, glx_backend))]
    GlxThenEgl,

    /// Prefer EGL and fallback to GLX.
    #[cfg(all(egl_backend, wgl_backend))]
    EglThenWgl,
    /// Prefer GLX and fallback to EGL.
    #[cfg(all(egl_backend, wgl_backend))]
    WglThenEgl,
}

/// A settings to control automatic display picking.
pub struct DisplayPicker {
    pub(crate) api_preference: DisplayApiPreference,
    #[cfg(glx_backend)]
    pub(crate) glx_error_registrator: Option<XlibErrorHookRegistrator>,
}

impl Default for DisplayPicker {
    #[cfg(all(egl_backend, glx_backend))]
    fn default() -> Self {
        Self { api_preference: DisplayApiPreference::Egl, glx_error_registrator: None }
    }

    #[cfg(all(egl_backend, not(glx_backend)))]
    fn default() -> Self {
        Self { api_preference: DisplayApiPreference::Egl }
    }

    #[cfg(all(glx_backend, not(egl_backend)))]
    fn default() -> Self {
        Self { api_preference: DisplayApiPreference::Glx, glx_error_registrator: None }
    }

    #[cfg(all(wgl_backend, not(egl_backend)))]
    fn default() -> Self {
        Self { api_preference: DisplayApiPreference::Wgl }
    }

    #[cfg(cgl_backend)]
    fn default() -> Self {
        Self { api_preference: DisplayApiPreference::Cgl }
    }
}

impl DisplayPicker {
    pub fn new() -> Self {
        Default::default()
    }

    /// The preference of the underlying system Api.
    pub fn with_api_preference(mut self, api_preference: DisplayApiPreference) -> Self {
        self.api_preference = api_preference;
        self
    }

    /// The hook to register glutin error handler in X11 error handling function.
    ///
    /// The hook registrator must be provided in case GLX will be used.
    #[cfg(glx_backend)]
    pub fn with_glx_error_registrator(
        mut self,
        error_registrator: XlibErrorHookRegistrator,
    ) -> Self {
        self.glx_error_registrator = Some(error_registrator);
        self
    }
}

pub trait GlDisplay: Sealed {
    type WindowSurface: GlSurface<WindowSurface>;
    type PixmapSurface: GlSurface<PixmapSurface>;
    type PbufferSurface: GlSurface<PbufferSurface>;
    type Config: GlConfig;
    type NotCurrentContext: NotCurrentGlContext;

    /// Find configuration matching the given `template`.
    fn find_configs(
        &self,
        template: ConfigTemplate,
    ) -> Option<Box<dyn Iterator<Item = Self::Config> + '_>>;

    /// Create the graphics platform context.
    fn create_context(
        &self,
        config: &Self::Config,
        context_attributes: &ContextAttributes,
    ) -> Result<Self::NotCurrentContext>;

    /// Create the surface that can be used to render into native window.
    ///
    /// # Safety
    ///
    /// The [`RawWindowHandle`] must point to a valid object.
    unsafe fn create_window_surface(
        &self,
        config: &Self::Config,
        surface_attributes: &SurfaceAttributes<WindowSurface>,
    ) -> Result<Self::WindowSurface>;

    /// Create the surfate that can be used to render into pbuffer.
    fn create_pbuffer_surface(
        &self,
        config: &Self::Config,
        surface_attributes: &SurfaceAttributes<PbufferSurface>,
    ) -> Result<Self::PbufferSurface>;

    /// Create the surface that can be used to render into pixmap.
    ///
    /// # Safety
    ///
    /// The [`NativePixmap`] must represent a valid native pixmap.
    unsafe fn create_pixmap_surface(
        &self,
        config: &Self::Config,
        surface_attributes: &SurfaceAttributes<PixmapSurface>,
    ) -> Result<Self::PixmapSurface>;
}

/// The graphics display to handle underlying graphics platform in a crossplatform way.
#[derive(Clone)]
pub enum Display {
    /// The EGL display.
    #[cfg(egl_backend)]
    Egl(EglDisplay),

    /// The GLX display.
    #[cfg(glx_backend)]
    Glx(GlxDisplay),

    /// The WGL display.
    #[cfg(wgl_backend)]
    Wgl(WglDisplay),

    /// The CGL display.
    #[cfg(cgl_backend)]
    Cgl(CglDisplay),
}

impl GlDisplay for Display {
    type WindowSurface = Surface<WindowSurface>;
    type PixmapSurface = Surface<PixmapSurface>;
    type PbufferSurface = Surface<PbufferSurface>;
    type Config = Config;
    type NotCurrentContext = NotCurrentContext;

    fn find_configs(
        &self,
        template: ConfigTemplate,
    ) -> Option<Box<dyn Iterator<Item = Self::Config> + '_>> {
        match self {
            #[cfg(egl_backend)]
            Self::Egl(display) => {
                Some(Box::new(display.find_configs(template)?.into_iter().map(Config::Egl)))
            }
            #[cfg(glx_backend)]
            Self::Glx(display) => {
                Some(Box::new(display.find_configs(template)?.into_iter().map(Config::Glx)))
            }
            #[cfg(wgl_backend)]
            Self::Wgl(display) => {
                Some(Box::new(display.find_configs(template)?.into_iter().map(Config::Wgl)))
            }
            #[cfg(cgl_backend)]
            Self::Cgl(display) => {
                Some(Box::new(display.find_configs(template)?.into_iter().map(Config::Cgl)))
            }
        }
    }

    fn create_context(
        &self,
        config: &Self::Config,
        context_attributes: &ContextAttributes,
    ) -> Result<Self::NotCurrentContext> {
        match (self, config) {
            #[cfg(egl_backend)]
            (Self::Egl(display), Config::Egl(config)) => {
                Ok(NotCurrentContext::Egl(display.create_context(config, context_attributes)?))
            }
            #[cfg(glx_backend)]
            (Self::Glx(display), Config::Glx(config)) => {
                Ok(NotCurrentContext::Glx(display.create_context(config, context_attributes)?))
            }
            #[cfg(wgl_backend)]
            (Self::Wgl(display), Config::Wgl(config)) => {
                Ok(NotCurrentContext::Wgl(display.create_context(config, context_attributes)?))
            }
            #[cfg(cgl_backend)]
            (Self::Cgl(display), Config::Cgl(config)) => {
                Ok(NotCurrentContext::Cgl(display.create_context(config, context_attributes)?))
            }
            _ => unreachable!(),
        }
    }

    unsafe fn create_window_surface(
        &self,
        config: &Self::Config,
        surface_attributes: &SurfaceAttributes<WindowSurface>,
    ) -> Result<Self::WindowSurface> {
        match (self, config) {
            #[cfg(egl_backend)]
            (Self::Egl(display), Config::Egl(config)) => {
                Ok(Surface::Egl(display.create_window_surface(config, surface_attributes)?))
            }
            #[cfg(glx_backend)]
            (Self::Glx(display), Config::Glx(config)) => {
                Ok(Surface::Glx(display.create_window_surface(config, surface_attributes)?))
            }
            #[cfg(wgl_backend)]
            (Self::Wgl(display), Config::Wgl(config)) => {
                Ok(Surface::Wgl(display.create_window_surface(config, surface_attributes)?))
            }
            #[cfg(cgl_backend)]
            (Self::Cgl(display), Config::Cgl(config)) => {
                Ok(Surface::Cgl(display.create_window_surface(config, surface_attributes)?))
            }
            _ => unreachable!(),
        }
    }

    fn create_pbuffer_surface(
        &self,
        config: &Self::Config,
        surface_attributes: &SurfaceAttributes<PbufferSurface>,
    ) -> Result<Self::PbufferSurface> {
        match (self, config) {
            #[cfg(egl_backend)]
            (Self::Egl(display), Config::Egl(config)) => {
                Ok(Surface::Egl(display.create_pbuffer_surface(config, surface_attributes)?))
            }
            #[cfg(glx_backend)]
            (Self::Glx(display), Config::Glx(config)) => {
                Ok(Surface::Glx(display.create_pbuffer_surface(config, surface_attributes)?))
            }
            #[cfg(wgl_backend)]
            (Self::Wgl(display), Config::Wgl(config)) => {
                Ok(Surface::Wgl(display.create_pbuffer_surface(config, surface_attributes)?))
            }
            #[cfg(cgl_backend)]
            (Self::Cgl(display), Config::Cgl(config)) => {
                Ok(Surface::Cgl(display.create_pbuffer_surface(config, surface_attributes)?))
            }
            _ => unreachable!(),
        }
    }

    unsafe fn create_pixmap_surface(
        &self,
        config: &Self::Config,
        surface_attributes: &SurfaceAttributes<PixmapSurface>,
    ) -> Result<Self::PixmapSurface> {
        match (self, config) {
            #[cfg(egl_backend)]
            (Self::Egl(display), Config::Egl(config)) => {
                Ok(Surface::Egl(display.create_pixmap_surface(config, surface_attributes)?))
            }
            #[cfg(glx_backend)]
            (Self::Glx(display), Config::Glx(config)) => {
                Ok(Surface::Glx(display.create_pixmap_surface(config, surface_attributes)?))
            }
            #[cfg(wgl_backend)]
            (Self::Wgl(display), Config::Wgl(config)) => {
                Ok(Surface::Wgl(display.create_pixmap_surface(config, surface_attributes)?))
            }
            #[cfg(cgl_backend)]
            (Self::Cgl(display), Config::Cgl(config)) => {
                Ok(Surface::Cgl(display.create_pixmap_surface(config, surface_attributes)?))
            }
            _ => unreachable!(),
        }
    }
}

impl Sealed for Display {}

impl Display {
    /// Create a graphics platform display from the given raw display handle.
    ///
    /// # Safety
    ///
    /// The `display` must point to the valid platform display.
    pub unsafe fn from_raw(display: RawDisplayHandle, picker: DisplayPicker) -> Result<Self> {
        #[cfg(glx_backend)]
        let registrator = picker
            .glx_error_registrator
            .expect("glx was requested, but error hook registrator wasn't provided.");

        match picker.api_preference {
            #[cfg(egl_backend)]
            DisplayApiPreference::Egl => Ok(Self::Egl(EglDisplay::from_raw(display)?)),
            #[cfg(glx_backend)]
            DisplayApiPreference::Glx => Ok(Self::Glx(GlxDisplay::from_raw(display, registrator)?)),
            #[cfg(wgl_backend)]
            DisplayApiPreference::Wgl => Ok(Self::Wgl(WglDisplay::from_raw(display)?)),
            #[cfg(cgl_backend)]
            DisplayApiPreference::Cgl => Ok(Self::Cgl(CglDisplay::from_raw(display)?)),

            #[cfg(all(egl_backend, glx_backend))]
            DisplayApiPreference::EglThenGlx => {
                if let Ok(display) = EglDisplay::from_raw(display) {
                    Ok(Self::Egl(display))
                } else {
                    Ok(Self::Glx(GlxDisplay::from_raw(display, registrator)?))
                }
            }
            #[cfg(all(egl_backend, glx_backend))]
            DisplayApiPreference::GlxThenEgl => {
                if let Ok(display) = GlxDisplay::from_raw(display, registrator) {
                    Ok(Self::Glx(display))
                } else {
                    Ok(Self::Egl(EglDisplay::from_raw(display)?))
                }
            }

            #[cfg(all(egl_backend, wgl_backend))]
            DisplayApiPreference::EglThenWgl => {
                if let Ok(display) = EglDisplay::from_raw(display) {
                    Ok(Self::Egl(display))
                } else {
                    Ok(Self::Wgl(WglDisplay::from_raw(display)?))
                }
            }
            #[cfg(all(egl_backend, wgl_backend))]
            DisplayApiPreference::WglThenEgl => {
                if let Ok(display) = WglDisplay::from_raw(display) {
                    Ok(Self::Wgl(display))
                } else {
                    Ok(Self::Egl(EglDisplay::from_raw(display)?))
                }
            }
        }
    }
}

pub trait GetGlDisplay: Sealed {
    type Target: GlDisplay;

    /// Obtain the GL display used to create a particular GL object.
    fn display(&self) -> Self::Target;
}

/// Raw GL platform display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawDisplay {
    /// Raw EGL display.
    #[cfg(egl_backend)]
    Egl(*const std::ffi::c_void),

    /// Raw GLX display.
    #[cfg(glx_backend)]
    Glx(*const std::ffi::c_void),

    /// TODO.
    #[cfg(wgl_backend)]
    Wgl(*const std::ffi::c_void),

    #[cfg(cgl_backend)]
    Cgl,
}

pub trait AsRawDisplay {
    /// A raw handle to the underlying api display.
    fn raw_display(&self) -> RawDisplay;
}

impl AsRawDisplay for Display {
    fn raw_display(&self) -> RawDisplay {
        dispatch_gl!(self; Self(display) => display.raw_display())
    }
}
