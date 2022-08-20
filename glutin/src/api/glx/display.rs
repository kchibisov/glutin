use std::collections::HashSet;
use std::ffi::CStr;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use raw_window_handle::RawDisplayHandle;

use glutin_glx_sys::glx;

use crate::config::ConfigTemplate;
use crate::display::{AsRawDisplay, RawDisplay};
use crate::error::{ErrorKind, Result};
use crate::prelude::*;
use crate::private::Sealed;
use crate::surface::{PbufferSurface, PixmapSurface, SurfaceAttributes, WindowSurface};

use super::config::Config;
use super::context::NotCurrentContext;
use super::surface::Surface;
use super::{Glx, GlxDisplay, GlxExtra, XlibErrorHookRegistrator, GLX, GLX_BASE_ERROR, GLX_EXTRA};

#[derive(Default, Clone, Copy)]
pub(crate) struct GlxVersion {
    pub(crate) major: i32,
    pub(crate) minor: i32,
}

#[derive(Clone)]
pub struct Display {
    pub(crate) inner: Arc<DisplayInner>,
}

pub(crate) struct DisplayInner {
    pub(crate) glx: &'static Glx,
    pub(crate) glx_extra: Option<&'static GlxExtra>,
    pub(crate) raw: GlxDisplay,
    pub(crate) screen: i32,
    /// Client GLX extensions.
    pub(crate) client_extensions: HashSet<&'static str>,
}

impl Display {
    /// Create GLX display.
    ///
    /// # Safety
    ///
    /// The `display` must point to the valid Xlib display.
    pub unsafe fn from_raw(
        display: RawDisplayHandle,
        error_hook_registrator: XlibErrorHookRegistrator,
    ) -> Result<Self> {
        // Don't load GLX when unsupported platform was requested.
        let (display, screen) = match display {
            RawDisplayHandle::Xlib(handle) => {
                (GlxDisplay(handle.display as *mut _), handle.screen as i32)
            }
            _ => return Err(ErrorKind::NotSupported.into()),
        };

        let glx = match GLX.as_ref() {
            Some(glx) => glx,
            None => return Err(ErrorKind::NotFound.into()),
        };

        // Set the base for errors comming from glx.
        let mut error_base = 0;
        let mut event_base = 0;
        if glx.QueryExtension(display.0, &mut error_base, &mut event_base) == 0 {
            // The glx extension isn't present.
            return Err(ErrorKind::InitializationFailed.into());
        }
        GLX_BASE_ERROR.store(error_base, Ordering::Relaxed);

        // This is completely ridiculous, but VirtualBox's OpenGL driver needs
        // some call handled by *it* (i.e. not Mesa) to occur before
        // anything else can happen. That is because VirtualBox's OpenGL
        // driver is going to apply binary patches to Mesa in the DLL
        // constructor and until it's loaded it won't have a chance to do that.
        //
        // The easiest way to do this is to just call `glXQueryVersion()` before
        // doing anything else. See: https://www.virtualbox.org/ticket/8293
        let mut version = GlxVersion::default();
        if glx.QueryVersion(display.0, &mut version.major, &mut version.minor) == 0 {
            return Err(ErrorKind::InitializationFailed.into());
        }

        if version.major < 1 && version.minor < 3 {
            return Err(ErrorKind::NotSupported.into());
        }

        // Register the error handling hook.
        error_hook_registrator(Box::new(super::glx_error_hook));

        let client_extensions = get_extensions(glx, display);

        let inner = Arc::new(DisplayInner {
            raw: display,
            glx,
            glx_extra: GLX_EXTRA.as_ref(),
            screen,
            client_extensions,
        });

        Ok(Self { inner })
    }
}

impl Sealed for Display {}

impl GlDisplay for Display {
    type WindowSurface = Surface<WindowSurface>;
    type PixmapSurface = Surface<PixmapSurface>;
    type PbufferSurface = Surface<PbufferSurface>;
    type Config = Config;
    type NotCurrentContext = NotCurrentContext;

    fn find_configs(
        &self,
        template: ConfigTemplate,
    ) -> Result<Box<dyn Iterator<Item = Self::Config> + '_>> {
        Self::find_configs(self, template)
    }

    unsafe fn create_window_surface(
        &self,
        config: &Self::Config,
        surface_attributes: &SurfaceAttributes<WindowSurface>,
    ) -> Result<Self::WindowSurface> {
        Self::create_window_surface(self, config, surface_attributes)
    }

    fn create_pbuffer_surface(
        &self,
        config: &Self::Config,
        surface_attributes: &SurfaceAttributes<PbufferSurface>,
    ) -> Result<Self::PbufferSurface> {
        Self::create_pbuffer_surface(self, config, surface_attributes)
    }

    fn create_context(
        &self,
        config: &Self::Config,
        context_attributes: &crate::context::ContextAttributes,
    ) -> Result<Self::NotCurrentContext> {
        Self::create_context(self, config, context_attributes)
    }

    unsafe fn create_pixmap_surface(
        &self,
        config: &Self::Config,
        surface_attributes: &SurfaceAttributes<PixmapSurface>,
    ) -> Result<Self::PixmapSurface> {
        Self::create_pixmap_surface(self, config, surface_attributes)
    }
}

impl AsRawDisplay for Display {
    fn raw_display(&self) -> RawDisplay {
        RawDisplay::Glx(self.inner.raw.cast())
    }
}

fn get_extensions(glx: &Glx, display: GlxDisplay) -> HashSet<&'static str> {
    unsafe {
        let extensions = glx.GetClientString(display.0, glx::EXTENSIONS as i32);
        if extensions.is_null() {
            return HashSet::new();
        }

        if let Ok(extensions) = CStr::from_ptr(extensions).to_str() {
            extensions.split(' ').collect::<HashSet<_>>()
        } else {
            HashSet::new()
        }
    }
}
