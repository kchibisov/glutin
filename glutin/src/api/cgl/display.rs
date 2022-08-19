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

#[derive(Clone)]
pub struct Display;

impl Display {
    /// Create CGL display.
    pub fn from_raw(display: RawDisplayHandle) -> Result<Self> {
        match display {
            RawDisplayHandle::AppKit(..) => Ok(Display),
            _ => Err(ErrorKind::NotSupported.into()),
        }
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

impl AsRawDisplay for Display {
    fn raw_display(&self) -> RawDisplay {
        RawDisplay::Cgl
    }
}
