use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::os::raw::{c_int, c_uint};

use raw_window_handle::RawWindowHandle;

use crate::config::GetGlConfig;
use crate::display::GetGlDisplay;
use crate::error::Result;
use crate::private::Sealed;
use crate::surface::{
    AsRawSurface, GlSurface, PbufferSurface, PixmapSurface, RawSurface, SurfaceAttributes,
    SurfaceType, SurfaceTypeTrait, WindowSurface,
};

use super::config::Config;
use super::context::PossiblyCurrentContext;
use super::display::Display;

impl Display {
    pub(crate) unsafe fn create_pixmap_surface(
        &self,
        config: &Config,
        surface_attributes: &SurfaceAttributes<PixmapSurface>,
    ) -> Result<Surface<PixmapSurface>> {
        todo!()
    }

    pub(crate) fn create_pbuffer_surface(
        &self,
        config: &Config,
        surface_attributes: &SurfaceAttributes<PbufferSurface>,
    ) -> Result<Surface<PbufferSurface>> {
        todo!()
    }

    pub(crate) unsafe fn create_window_surface(
        &self,
        config: &Config,
        surface_attributes: &SurfaceAttributes<WindowSurface>,
    ) -> Result<Surface<WindowSurface>> {
        todo!()
    }
}

pub struct Surface<T: SurfaceTypeTrait> {
    display: Display,
    config: Config,
    _ty: PhantomData<T>,
}

impl<T: SurfaceTypeTrait> Sealed for Surface<T> {}

impl<T: SurfaceTypeTrait> AsRawSurface for Surface<T> {
    fn raw_surface(&self) -> RawSurface {
        todo!()
    }
}

impl<T: SurfaceTypeTrait> GlSurface<T> for Surface<T> {
    type SurfaceType = T;
    type Context = PossiblyCurrentContext;

    fn buffer_age(&self) -> u32 {
        todo!()
    }

    fn width(&self) -> Option<u32> {
        todo!()
    }

    fn height(&self) -> Option<u32> {
        todo!()
    }

    fn is_single_buffered(&self) -> bool {
        todo!()
    }

    fn swap_buffers(&self, _context: &Self::Context) -> Result<()> {
        todo!()
    }

    fn is_current(&self, _context: &Self::Context) -> bool {
        todo!()
    }

    fn is_current_draw(&self, _context: &Self::Context) -> bool {
        todo!()
    }

    fn is_current_read(&self, _context: &Self::Context) -> bool {
        todo!()
    }

    fn resize(&self, _context: &Self::Context, _width: NonZeroU32, _height: NonZeroU32) {
        // This isn't supported with GLXDrawable.
    }
}

impl<T: SurfaceTypeTrait> GetGlConfig for Surface<T> {
    type Target = Config;
    fn config(&self) -> Self::Target {
        self.config.clone()
    }
}

impl<T: SurfaceTypeTrait> GetGlDisplay for Surface<T> {
    type Target = Display;
    fn display(&self) -> Self::Target {
        self.display.clone()
    }
}

impl<T: SurfaceTypeTrait> Surface<T> {
    fn raw_attribute(&self, attr: c_int) -> c_uint {
        todo!()
    }
}

impl<T: SurfaceTypeTrait> Drop for Surface<T> {
    fn drop(&mut self) {
        todo!()
    }
}
