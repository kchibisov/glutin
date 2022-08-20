use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::os::raw::{c_int, c_uint};

use glutin_wgl_sys::wgl;
use glutin_wgl_sys::wgl::types::HGLRC;
use raw_window_handle::RawWindowHandle;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::{self as gdi, HDC};
use windows_sys::Win32::Graphics::OpenGL::{self as gl, PIXELFORMATDESCRIPTOR};

use crate::config::GetGlConfig;
use crate::display::GetGlDisplay;
use crate::error::{ErrorKind, Result};
use crate::prelude::*;
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
        let hwnd = match surface_attributes.raw_window_handle.as_ref().unwrap() {
            handle @ RawWindowHandle::Win32(window_handle) => {
                let _ = config.apply_on_native_window(handle);
                window_handle.hwnd as HWND
            }
            _ => return Err(ErrorKind::NotSupported.into()),
        };

        let surface =
            Surface { display: self.clone(), config: config.clone(), hwnd, _ty: PhantomData };

        Ok(surface)
    }
}

pub struct Surface<T: SurfaceTypeTrait> {
    display: Display,
    config: Config,
    pub(crate) hwnd: HWND,
    _ty: PhantomData<T>,
}

impl<T: SurfaceTypeTrait> Sealed for Surface<T> {}

impl<T: SurfaceTypeTrait> AsRawSurface for Surface<T> {
    fn raw_surface(&self) -> RawSurface {
        RawSurface::Wgl(self.hwnd as _)
    }
}

impl<T: SurfaceTypeTrait> GlSurface<T> for Surface<T> {
    type SurfaceType = T;
    type Context = PossiblyCurrentContext;

    fn buffer_age(&self) -> u32 {
        0
    }

    fn width(&self) -> Option<u32> {
        None
    }

    fn height(&self) -> Option<u32> {
        None
    }

    fn is_single_buffered(&self) -> bool {
        // TODO
        false
    }

    fn swap_buffers(&self, _context: &Self::Context) -> Result<()> {
        unsafe {
            let hdc = gdi::GetDC(self.hwnd);
            gl::SwapBuffers(hdc);
            Ok(())
        }
    }

    fn is_current(&self, context: &Self::Context) -> bool {
        context.is_current()
    }

    fn is_current_draw(&self, context: &Self::Context) -> bool {
        context.is_current()
    }

    fn is_current_read(&self, context: &Self::Context) -> bool {
        context.is_current()
    }

    fn resize(&self, _context: &Self::Context, _width: NonZeroU32, _height: NonZeroU32) {
        // This isn't supported with WGL.
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
        // This line intentionally left blank.
    }
}
