// TODO add creation checks
use std::ffi;
use std::marker::PhantomData;
use std::num::NonZeroU32;

use glutin_egl_sys::egl;
use glutin_egl_sys::egl::types::{EGLAttrib, EGLSurface, EGLint};

use raw_window_handle::RawWindowHandle;

use crate::config::GetGlConfig;
use crate::display::GetGlDisplay;
use crate::error::{ErrorKind, Result};
use crate::prelude::*;
use crate::private::Sealed;
use crate::surface::{
    AsRawSurface, PbufferSurface, PixmapSurface, RawSurface, SurfaceAttributes, SurfaceTypeTrait,
    WindowSurface,
};

use super::config::Config;
use super::context::PossiblyCurrentContext;
use super::display::Display;

const ATTR_SIZE_HINT: usize = 8;

impl Display {
    pub(crate) fn create_pbuffer_surface(
        &self,
        config: &Config,
        surface_attributes: &SurfaceAttributes<PbufferSurface>,
    ) -> Result<Surface<PbufferSurface>> {
        let width = surface_attributes.width.unwrap();
        let height = surface_attributes.height.unwrap();

        // XXX Window surface is using `EGLAttrib` and not `EGLint`.
        let mut attrs = Vec::<EGLAttrib>::with_capacity(ATTR_SIZE_HINT);

        // Add dimensions.
        attrs.push(egl::WIDTH as EGLAttrib);
        attrs.push(width.get() as EGLAttrib);

        attrs.push(egl::HEIGHT as EGLAttrib);
        attrs.push(height.get() as EGLAttrib);

        // Add information about render buffer.
        attrs.push(egl::RENDER_BUFFER as EGLAttrib);
        let buffer =
            if surface_attributes.single_buffer { egl::SINGLE_BUFFER } else { egl::BACK_BUFFER }
                as EGLAttrib;
        attrs.push(buffer);

        // Push `egl::NONE` to terminate the list.
        attrs.push(egl::NONE as EGLAttrib);

        unsafe {
            let config = config.clone();
            let surface = Self::check_surface_error(self.inner.egl.CreatePbufferSurface(
                self.inner.raw,
                config.inner.raw,
                attrs.as_ptr() as *const _,
            ))?;

            Ok(Surface {
                display: self.clone(),
                native_window: None,
                config,
                raw: surface,
                _ty: PhantomData,
            })
        }
    }

    pub(crate) unsafe fn create_pixmap_surface(
        &self,
        config: &Config,
        surface_attributes: &SurfaceAttributes<PixmapSurface>,
    ) -> Result<Surface<PixmapSurface>> {
        let native_pixmap = surface_attributes.native_pixmap.as_ref().unwrap();

        let mut attrs = Vec::<EGLAttrib>::with_capacity(ATTR_SIZE_HINT);

        if surface_attributes.srgb.is_some()
            && self.inner.client_extensions.contains("EGL_KHR_gl_colorspace")
        {
            attrs.push(egl::GL_COLORSPACE as EGLAttrib);
            let colorspace = match surface_attributes.srgb {
                Some(true) => egl::GL_COLORSPACE_SRGB as EGLAttrib,
                _ => egl::GL_COLORSPACE_LINEAR as EGLAttrib,
            };
            attrs.push(colorspace);
        }

        // Push `egl::NONE` to terminate the list.
        attrs.push(egl::NONE as EGLAttrib);

        let config = config.clone();
        let surface = Self::check_surface_error(self.inner.egl.CreatePlatformPixmapSurface(
            self.inner.raw,
            config.inner.raw,
            native_pixmap.raw(),
            attrs.as_ptr(),
        ))?;

        Ok(Surface {
            display: self.clone(),
            config,
            native_window: None,
            raw: surface,
            _ty: PhantomData,
        })
    }

    pub(crate) unsafe fn create_window_surface(
        &self,
        config: &Config,
        surface_attributes: &SurfaceAttributes<WindowSurface>,
    ) -> Result<Surface<WindowSurface>> {
        // Create native window.
        let native_window = NativeWindow::new(
            surface_attributes.width.unwrap(),
            surface_attributes.height.unwrap(),
            surface_attributes.raw_window_handle.as_ref().unwrap(),
        )?;

        // XXX Window surface is using `EGLAttrib` and not `EGLint`.
        let mut attrs = Vec::<EGLAttrib>::with_capacity(ATTR_SIZE_HINT);

        // Add information about render buffer.
        attrs.push(egl::RENDER_BUFFER as EGLAttrib);
        let buffer =
            if surface_attributes.single_buffer { egl::SINGLE_BUFFER } else { egl::BACK_BUFFER }
                as EGLAttrib;
        attrs.push(buffer);

        // // Add colorspace if the extension is present.
        if surface_attributes.srgb.is_some()
            && self.inner.client_extensions.contains("EGL_KHR_gl_colorspace")
        {
            attrs.push(egl::GL_COLORSPACE as EGLAttrib);
            let colorspace = match surface_attributes.srgb {
                Some(true) => egl::GL_COLORSPACE_SRGB as EGLAttrib,
                _ => egl::GL_COLORSPACE_LINEAR as EGLAttrib,
            };
            attrs.push(colorspace);
        }

        // Push `egl::NONE` to terminate the list.
        attrs.push(egl::NONE as EGLAttrib);

        let config = config.clone();
        let surface = Self::check_surface_error(self.inner.egl.CreatePlatformWindowSurface(
            self.inner.raw,
            config.inner.raw,
            native_window.raw(),
            attrs.as_ptr() as *const _,
        ))?;

        Ok(Surface {
            display: self.clone(),
            config,
            native_window: Some(native_window),
            raw: surface,
            _ty: PhantomData,
        })
    }

    fn check_surface_error(surface: EGLSurface) -> Result<EGLSurface> {
        if surface == egl::NO_SURFACE {
            Err(super::check_error().err().unwrap())
        } else {
            Ok(surface)
        }
    }
}

pub struct Surface<T: SurfaceTypeTrait> {
    display: Display,
    config: Config,
    pub(crate) raw: EGLSurface,
    native_window: Option<NativeWindow>,
    _ty: PhantomData<T>,
}

impl<T: SurfaceTypeTrait> GlSurface<T> for Surface<T> {
    type SurfaceType = T;
    type Context = PossiblyCurrentContext;

    fn buffer_age(&self) -> u32 {
        self.raw_attribute(egl::BUFFER_AGE_EXT as EGLint) as u32
    }

    fn width(&self) -> Option<u32> {
        Some(self.raw_attribute(egl::HEIGHT as EGLint) as u32)
    }

    fn height(&self) -> Option<u32> {
        Some(self.raw_attribute(egl::HEIGHT as EGLint) as u32)
    }

    fn is_single_buffered(&self) -> bool {
        self.raw_attribute(egl::RENDER_BUFFER as EGLint) != egl::SINGLE_BUFFER as i32
    }

    fn swap_buffers(&self, _context: &Self::Context) -> Result<()> {
        unsafe {
            if self.display.inner.egl.SwapBuffers(self.display.inner.raw, self.raw) == egl::FALSE {
                super::check_error()
            } else {
                Ok(())
            }
        }
    }

    fn is_current(&self, context: &Self::Context) -> bool {
        self.is_current_draw(context) && self.is_current_read(context)
    }

    fn is_current_draw(&self, _context: &Self::Context) -> bool {
        unsafe { self.display.inner.egl.GetCurrentSurface(egl::DRAW as EGLint) == self.raw }
    }

    fn is_current_read(&self, _context: &Self::Context) -> bool {
        unsafe { self.display.inner.egl.GetCurrentSurface(egl::READ as EGLint) == self.raw }
    }

    fn resize(&self, _context: &Self::Context, width: NonZeroU32, height: NonZeroU32) {
        self.native_window.as_ref().unwrap().resize(width, height)
    }
}

impl<T: SurfaceTypeTrait> Sealed for Surface<T> {}

impl<T: SurfaceTypeTrait> GetGlDisplay for Surface<T> {
    type Target = Display;
    fn display(&self) -> Self::Target {
        self.display.clone()
    }
}

impl<T: SurfaceTypeTrait> GetGlConfig for Surface<T> {
    type Target = Config;
    fn config(&self) -> Self::Target {
        self.config.clone()
    }
}

impl<T: SurfaceTypeTrait> AsRawSurface for Surface<T> {
    fn raw_surface(&self) -> RawSurface {
        RawSurface::Egl(self.raw)
    }
}

// The damage rect that is being used in [`Surface::swap_buffers_with_damage`]. The origin is in
// the bottom left of the surface.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DamageRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl DamageRect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { x, y, width, height }
    }
}

impl<T: SurfaceTypeTrait> Surface<T> {
    /// Swaps the underlying back buffers when the surfate is not single buffered and pass the
    /// [`DamageRect`] information to the system compositor. Providing empty slice will result in
    /// damaging the entire surface.
    ///
    /// This Api doesn't do any parital rendering, it basically provides the hints for the system
    /// compositor.
    pub fn swap_buffers_with_damage(
        &self,
        _context: &PossiblyCurrentContext,
        rects: &[DamageRect],
    ) -> Result<()> {
        unsafe {
            if self.display.inner.egl.SwapBuffersWithDamageKHR(
                self.display.inner.raw,
                self.raw,
                rects.as_ptr() as *mut _,
                (rects.len() * 4) as _,
            ) == egl::FALSE
            {
                super::check_error()
            } else {
                Ok(())
            }
        }
    }

    fn raw_attribute(&self, attr: EGLint) -> EGLint {
        unsafe {
            let mut value = 0;
            self.display.inner.egl.QuerySurface(self.display.inner.raw, self.raw, attr, &mut value);
            value
        }
    }
}

impl<T: SurfaceTypeTrait> Drop for Surface<T> {
    fn drop(&mut self) {
        unsafe {
            self.display.inner.egl.DestroySurface(self.display.inner.raw, self.raw);
        }
    }
}

#[cfg(wayland_platform)]
use wayland_sys::{egl::*, ffi_dispatch};

enum NativeWindow {
    #[cfg(wayland_platform)]
    Wayland(*mut ffi::c_void),

    #[cfg(x11_platform)]
    Xlib(u64),

    Android(*mut ffi::c_void),

    Win32(*mut ffi::c_void),

    Gbm(*mut ffi::c_void),
}

impl NativeWindow {
    pub(crate) fn new(
        _width: NonZeroU32,
        _height: NonZeroU32,
        raw_window_handle: &RawWindowHandle,
    ) -> Result<Self> {
        let native_window = match raw_window_handle {
            #[cfg(wayland_platform)]
            RawWindowHandle::Wayland(window_handle) => unsafe {
                let ptr = ffi_dispatch!(
                    WAYLAND_EGL_HANDLE,
                    wl_egl_window_create,
                    window_handle.surface.cast(),
                    _width.get() as _,
                    _height.get() as _
                );
                if ptr.is_null() {
                    return Err(ErrorKind::OutOfMemory.into());
                }
                Self::Wayland(ptr.cast())
            },
            #[cfg(x11_platform)]
            RawWindowHandle::Xlib(window_handle) => Self::Xlib(window_handle.window as u64),
            RawWindowHandle::AndroidNdk(window_handle) => {
                Self::Android(window_handle.a_native_window)
            }
            RawWindowHandle::Win32(window_hanlde) => Self::Win32(window_hanlde.hwnd),
            RawWindowHandle::Gbm(window_handle) => Self::Gbm(window_handle.gbm_surface),
            _ => return Err(ErrorKind::NotSupported.into()),
        };

        Ok(native_window)
    }

    pub(crate) fn resize(&self, _width: NonZeroU32, _height: NonZeroU32) {
        match self {
            #[cfg(wayland_platform)]
            Self::Wayland(wl_egl_surface) => unsafe {
                ffi_dispatch!(
                    WAYLAND_EGL_HANDLE,
                    wl_egl_window_resize,
                    *wl_egl_surface as _,
                    _width.get() as _,
                    _height.get() as _,
                    0,
                    0
                )
            },
            #[cfg(x11_platform)]
            Self::Xlib(_) => (),
            Self::Android(_) => (),
            Self::Win32(_) => (),
            Self::Gbm(_) => (),
        }
    }

    pub(crate) fn raw(&self) -> *mut ffi::c_void {
        match self {
            #[cfg(wayland_platform)]
            Self::Wayland(wl_egl_surface) => *wl_egl_surface,
            #[cfg(x11_platform)]
            Self::Xlib(window_id) => window_id as *const _ as *mut ffi::c_void,
            Self::Win32(hwnd) => *hwnd,
            Self::Android(a_native_window) => *a_native_window,
            Self::Gbm(gbm_surface) => *gbm_surface,
        }
    }
}

#[cfg(wayland_platform)]
impl Drop for NativeWindow {
    fn drop(&mut self) {
        unsafe {
            if let Self::Wayland(wl_egl_window) = self {
                ffi_dispatch!(WAYLAND_EGL_HANDLE, wl_egl_window_destroy, wl_egl_window.cast());
            }
        }
    }
}
