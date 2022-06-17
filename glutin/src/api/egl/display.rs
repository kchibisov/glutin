use std::collections::HashSet;
use std::ffi::CStr;
use std::sync::Arc;

use glutin_egl_sys::egl;
use glutin_egl_sys::egl::types::{EGLAttrib, EGLDisplay, EGLint};

use once_cell::sync::OnceCell;

use raw_window_handle::RawDisplayHandle;

use crate::config::ConfigTemplate;
use crate::display::{AsRawDisplay, RawDisplay};
use crate::error::{ErrorKind, Result};
use crate::prelude::*;
use crate::private::Sealed;
use crate::surface::{PbufferSurface, PixmapSurface, SurfaceAttributes, WindowSurface};

use super::config::Config;
use super::context::NotCurrentContext;
use super::surface::Surface;

use super::{Egl, EGL};

/// Extensions that don't require any display.
static NO_DISPLAY_EXTENSIONS: OnceCell<HashSet<&'static str>> = OnceCell::new();

pub struct EglDisplay(EGLDisplay);

// Representation of EGL version.
#[derive(Default, Clone, Copy)]
pub(crate) struct EglVersion {
    pub(crate) major: i32,
    pub(crate) minor: i32,
}

/// A Display to wrap EGLDisplay and its supported extensions.
#[derive(Clone)]
pub struct Display {
    // Inner display to simplify passing it around.
    pub(crate) inner: Arc<DisplayInner>,
}

pub(crate) struct DisplayInner {
    /// Pointer to the EGL handler to simplify API calls.
    pub(crate) egl: &'static Egl,

    /// Pointer to the egl display.
    pub(crate) raw: EGLDisplay,

    /// The version of the egl library.
    pub(crate) version: EglVersion,

    /// Client EGL extensions.
    pub(crate) client_extensions: HashSet<&'static str>,

    /// The raw display used to create EGL display.
    pub(crate) raw_display: RawDisplayHandle,
}

impl Display {
    /// Create EGL display.
    ///
    /// # Safety
    ///
    /// `RawDisplay` must point to a valid system display.
    pub unsafe fn from_raw(raw_display: RawDisplayHandle) -> Result<Self> {
        let egl = match EGL.as_ref() {
            Some(egl) => egl,
            None => return Err(ErrorKind::NotFound.into()),
        };

        NO_DISPLAY_EXTENSIONS.get_or_init(|| get_extensions(egl, egl::NO_DISPLAY));

        // Create a EGL display by chaining all display creation functions aborting on
        // `EGL_BAD_ATTRIBUTE`.
        let display = Self::get_platform_display(egl, raw_display)
            .or_else(|err| {
                if err.error_kind() == ErrorKind::BadAttribute {
                    Err(err)
                } else {
                    Self::get_platform_display_ext(egl, raw_display)
                }
            })
            .or_else(|err| {
                if err.error_kind() == ErrorKind::BadAttribute {
                    Err(err)
                } else {
                    Self::get_display(egl, raw_display)
                }
            })?;

        let mut version = EglVersion::default();
        if egl.Initialize(display, &mut version.major, &mut version.minor) == egl::FALSE {
            return Err(super::check_error().err().unwrap());
        }

        // Load extensions.
        let client_extensions = get_extensions(egl, display);

        let inner =
            Arc::new(DisplayInner { egl, raw: display, raw_display, version, client_extensions });
        Ok(Self { inner })
    }

    fn get_platform_display(egl: &Egl, display: RawDisplayHandle) -> Result<EGLDisplay> {
        if !egl.GetPlatformDisplay.is_loaded() {
            return Err(ErrorKind::NotSupported.into());
        }

        let extensions = NO_DISPLAY_EXTENSIONS.get().unwrap();

        let mut attrs = Vec::<EGLAttrib>::new();
        let (platform, mut display) = match display {
            #[cfg(feature = "wayland")]
            RawDisplayHandle::Wayland(handle)
                if extensions.contains("EGL_KHR_platform_wayland") =>
            {
                (egl::PLATFORM_WAYLAND_KHR, handle.display)
            }
            #[cfg(feature = "x11")]
            RawDisplayHandle::Xlib(handle) if extensions.contains("EGL_KHR_platform_x11") => {
                attrs.push(egl::PLATFORM_X11_SCREEN_KHR as EGLAttrib);
                attrs.push(handle.screen as EGLAttrib);
                (egl::PLATFORM_X11_KHR, handle.display)
            }
            RawDisplayHandle::Gbm(handle) if extensions.contains("EGL_KHR_platform_gbm") => {
                (egl::PLATFORM_GBM_KHR, handle.gbm_device)
            }
            RawDisplayHandle::Android(_) if extensions.contains("EGL_KHR_platform_android") => {
                (egl::PLATFORM_ANDROID_KHR, egl::DEFAULT_DISPLAY as *mut _)
            }
            _ => return Err(ErrorKind::NotSupported.into()),
        };

        // Be explicit here.
        if display.is_null() {
            display = egl::DEFAULT_DISPLAY as *mut _;
        }

        // Push `egl::NONE` to terminate the list.
        attrs.push(egl::NONE as EGLAttrib);

        let display =
            unsafe { egl.GetPlatformDisplay(platform, display as *mut _, attrs.as_ptr()) };

        Self::check_display_error(display)
    }

    fn get_platform_display_ext(egl: &Egl, display: RawDisplayHandle) -> Result<EGLDisplay> {
        if !egl.GetPlatformDisplayEXT.is_loaded() {
            return Err(ErrorKind::NotSupported.into());
        }

        let extensions = NO_DISPLAY_EXTENSIONS.get().unwrap();

        let mut attrs = Vec::<EGLint>::new();
        let (platform, mut display) = match display {
            #[cfg(feature = "wayland")]
            RawDisplayHandle::Wayland(handle)
                if extensions.contains("EGL_EXT_platform_wayland") =>
            {
                (egl::PLATFORM_WAYLAND_EXT, handle.display)
            }
            #[cfg(feature = "x11")]
            RawDisplayHandle::Xlib(handle) if extensions.contains("EGL_EXT_platform_x11") => {
                attrs.push(egl::PLATFORM_X11_SCREEN_EXT as EGLint);
                attrs.push(handle.screen as EGLint);
                (egl::PLATFORM_X11_EXT, handle.display)
            }
            RawDisplayHandle::Gbm(handle) if extensions.contains("EGL_MESA_platform_gbm") => {
                (egl::PLATFORM_GBM_MESA, handle.gbm_device)
            }
            _ => return Err(ErrorKind::NotSupported.into()),
        };

        // Be explicit here.
        if display.is_null() {
            display = egl::DEFAULT_DISPLAY as *mut _;
        }

        // Push `egl::NONE` to terminate the list.
        attrs.push(egl::NONE as EGLint);

        let display =
            unsafe { egl.GetPlatformDisplayEXT(platform, display as *mut _, attrs.as_ptr()) };

        Self::check_display_error(display)
    }

    fn get_display(egl: &Egl, display: RawDisplayHandle) -> Result<EGLDisplay> {
        let mut display = match display {
            RawDisplayHandle::Gbm(handle) => handle.gbm_device,
            #[cfg(feature = "x11")]
            RawDisplayHandle::Xlib(handle) => handle.display,
            RawDisplayHandle::Android(_) => egl::DEFAULT_DISPLAY as *mut _,
            _ => return Err(ErrorKind::NotSupported.into()),
        };

        if display.is_null() {
            display = egl::DEFAULT_DISPLAY as *mut _;
        }

        let display = unsafe { egl.GetDisplay(display) };
        Self::check_display_error(display)
    }

    fn check_display_error(display: EGLDisplay) -> Result<EGLDisplay> {
        if display == egl::NO_DISPLAY {
            Err(super::check_error().err().unwrap())
        } else {
            Ok(display)
        }
    }
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

impl Sealed for Display {}

impl AsRawDisplay for Display {
    fn raw_display(&self) -> RawDisplay {
        RawDisplay::Egl(self.inner.raw)
    }
}

/// Collect egl extensions for the given `display`.
fn get_extensions(egl: &Egl, display: EGLDisplay) -> HashSet<&'static str> {
    unsafe {
        let extensions = egl.QueryString(display, egl::EXTENSIONS as i32);
        if extensions.is_null() {
            return HashSet::new();
        }

        if let Ok(extensions) = CStr::from_ptr(extensions).to_str() {
            extensions.split(' ').collect::<HashSet<&'static str>>()
        } else {
            HashSet::new()
        }
    }
}
