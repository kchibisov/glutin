//! A cross platform GL surface representation.

use std::ffi;
use std::marker::PhantomData;
use std::num::NonZeroU32;

use raw_window_handle::RawWindowHandle;

use crate::dispatch_gl;
use crate::display::{Display, GetGlDisplay};
use crate::error::Result;
use crate::private::Sealed;

#[cfg(cgl_backend)]
use crate::api::cgl::surface::Surface as CglSurface;
#[cfg(egl_backend)]
use crate::api::egl::surface::Surface as EglSurface;
#[cfg(glx_backend)]
use crate::api::glx::surface::Surface as GlxSurface;
#[cfg(wgl_backend)]
use crate::api::wgl::surface::Surface as WglSurface;

// The underlying type of the surface.
#[derive(Debug, Clone, Copy)]
pub enum SurfaceType {
    // The window surface.
    Window,

    // Pixmap surface.
    Pixmap,

    // Pbuffer surface.
    Pbuffer,
}

/// Marker that used to type-gate methods for window.
#[derive(Default, Debug, Clone, Copy)]
pub struct WindowSurface;

/// Marker that used to type-gate methods for pbuffer.
#[derive(Default, Debug, Clone, Copy)]
pub struct PbufferSurface;

/// Marker that used to type-gate methods for pixmap.
#[derive(Default, Debug, Clone, Copy)]
pub struct PixmapSurface;

/// Marker indicating that the surface could be resized.
pub trait ResizeableSurface: Sealed {}

impl SurfaceTypeTrait for WindowSurface {
    fn surface_type() -> SurfaceType {
        SurfaceType::Window
    }
}

impl SurfaceTypeTrait for PbufferSurface {
    fn surface_type() -> SurfaceType {
        SurfaceType::Pbuffer
    }
}

impl SurfaceTypeTrait for PixmapSurface {
    fn surface_type() -> SurfaceType {
        SurfaceType::Pixmap
    }
}

pub trait SurfaceTypeTrait {
    fn surface_type() -> SurfaceType;
}

/// Attributes which are used for creating a paritcular surface.
#[derive(Default, Debug, Clone)]
pub struct SurfaceAttributes<T: SurfaceTypeTrait> {
    pub(crate) srgb: Option<bool>,
    pub(crate) single_buffer: bool,
    pub(crate) width: Option<NonZeroU32>,
    pub(crate) height: Option<NonZeroU32>,
    pub(crate) largest_pbuffer: bool,
    pub(crate) raw_window_handle: Option<RawWindowHandle>,
    pub(crate) native_pixmap: Option<NativePixmap>,
    _ty: PhantomData<T>,
}

/// Builder to get the required set of attributes initialized before hand.
#[derive(Default)]
pub struct SurfaceAttributesBuilder<T: SurfaceTypeTrait + Default> {
    attributes: SurfaceAttributes<T>,
}

impl<T: SurfaceTypeTrait + Default> SurfaceAttributesBuilder<T> {
    pub fn new() -> Self {
        Default::default()
    }

    /// Specify whether the surface should support srgb or not. Passing `None` means you don't care.
    ///
    /// # Api spicific.
    ///
    /// This only controls EGL surfaces, since the rest are using context for that.
    pub fn with_srgb(mut self, srgb: Option<bool>) -> Self {
        self.attributes.srgb = srgb;
        self
    }
}

impl SurfaceAttributesBuilder<PixmapSurface> {
    pub fn build(mut self, native_pixmap: NativePixmap) -> SurfaceAttributes<PixmapSurface> {
        self.attributes.native_pixmap = Some(native_pixmap);
        self.attributes
    }
}

impl SurfaceAttributesBuilder<WindowSurface> {
    /// Specify whether the single buffer should be used instead of double buffering. This doesn't
    /// guatantee that the resulted buffer will have only single buffer, to know that the single
    /// buffer is actually used query the created surface with [`Surface::is_single_buffered`].
    ///
    /// The surface is requested as double buffered by default.
    ///
    /// # Api spicific.
    ///
    /// This is EGL specific, since the rest are using it on the context.
    pub fn with_single_buffer(mut self, single_buffer: bool) -> Self {
        self.attributes.single_buffer = single_buffer;
        self
    }

    pub fn build(
        mut self,
        raw_window_handle: RawWindowHandle,
        width: NonZeroU32,
        height: NonZeroU32,
    ) -> SurfaceAttributes<WindowSurface> {
        self.attributes.raw_window_handle = Some(raw_window_handle);
        self.attributes.width = Some(width);
        self.attributes.height = Some(height);
        self.attributes
    }
}

impl SurfaceAttributesBuilder<PbufferSurface> {
    /// Request the largest pbuffer.
    pub fn with_largest_pbuffer(mut self, largest_pbuffer: bool) -> Self {
        self.attributes.largest_pbuffer = largest_pbuffer;
        self
    }

    /// The same as in [SurfaceAttributesBuilder::<WindowSurface>::with_single_buffer"].
    pub fn with_single_buffer(mut self, single_buffer: bool) -> Self {
        self.attributes.single_buffer = single_buffer;
        self
    }

    pub fn build(
        mut self,
        width: NonZeroU32,
        height: NonZeroU32,
    ) -> SurfaceAttributes<PbufferSurface> {
        self.attributes.width = Some(width);
        self.attributes.height = Some(height);
        self.attributes
    }
}

pub trait GlSurface<T: SurfaceTypeTrait>: Sealed {
    type SurfaceType: SurfaceTypeTrait;

    /// The age of the back buffer of that surface. The `0` indicates that the buffer is either
    /// a new one or we failed to get the information about its age. In both cases you must redraw
    /// the entire buffer.
    fn buffer_age(&self) -> u32;

    /// The width of the underlying surface.
    fn width(&self) -> Option<u32>;

    /// The height of the underlying surface.
    fn height(&self) -> Option<u32>;

    /// Check whether the surface is single buffered.
    fn is_single_buffered(&self) -> bool;

    /// Swaps the underlying back buffers when the surfate is not single buffered.
    fn swap_buffers(&self) -> Result<()>;

    /// Check whether the surface is current on to the curren thread.
    fn is_current(&self) -> bool;

    /// Check wherher the surface is current draw surface to the current thread.
    fn is_current_draw(&self) -> bool;

    /// Check wherher the surface is current read surface to the current thread.
    fn is_current_read(&self) -> bool;

    /// Resize the surface to the new size.
    ///
    /// This call is for compatibility reasons, on most platforms it's a no-op.
    ///
    /// # Platform specific
    ///
    /// **Wayland:** - resizes the surface.
    /// **Other:** - no op.
    fn resize(&self, width: NonZeroU32, height: NonZeroU32)
    where
        Self::SurfaceType: ResizeableSurface;
}

pub enum Surface<T: SurfaceTypeTrait> {
    #[cfg(egl_backend)]
    Egl(EglSurface<T>),

    #[cfg(glx_backend)]
    Glx(GlxSurface<T>),

    #[cfg(wgl_backend)]
    Wgl(WglSurface<T>),

    #[cfg(cgl_backend)]
    Cgl(CglSurface<T>),
}

impl<T: SurfaceTypeTrait> GlSurface<T> for Surface<T> {
    type SurfaceType = T;

    fn buffer_age(&self) -> u32 {
        dispatch_gl!(self; Self(surface) => surface.buffer_age())
    }

    fn width(&self) -> Option<u32> {
        dispatch_gl!(self; Self(surface) => surface.width())
    }

    fn height(&self) -> Option<u32> {
        dispatch_gl!(self; Self(surface) => surface.height())
    }

    fn is_single_buffered(&self) -> bool {
        dispatch_gl!(self; Self(surface) => surface.is_single_buffered())
    }

    fn swap_buffers(&self) -> Result<()> {
        dispatch_gl!(self; Self(surface) => surface.swap_buffers())
    }

    fn is_current(&self) -> bool {
        dispatch_gl!(self; Self(surface) => surface.is_current())
    }

    fn is_current_draw(&self) -> bool {
        dispatch_gl!(self; Self(surface) => surface.is_current_draw())
    }

    fn is_current_read(&self) -> bool {
        dispatch_gl!(self; Self(surface) => surface.is_current_read())
    }

    fn resize(&self, width: NonZeroU32, height: NonZeroU32)
    where
        Self::SurfaceType: ResizeableSurface,
    {
        dispatch_gl!(self; Self(surface) => surface.resize(width, height))
    }
}

impl<T: SurfaceTypeTrait> Sealed for Surface<T> {}
impl Sealed for WindowSurface {}
impl Sealed for PixmapSurface {}
impl Sealed for PbufferSurface {}

impl<T: SurfaceTypeTrait> GetGlDisplay for Surface<T> {
    type Target = Display;
    fn display(&self) -> Self::Target {
        dispatch_gl!(self; Self(surface) => surface.display(); as Display)
    }
}

impl ResizeableSurface for WindowSurface {}

// A platform native pixmap.
#[derive(Debug, Clone, Copy)]
pub enum NativePixmap {
    /// XID of X11 pixmap.  
    X11Pixmap(u64),

    /// HBITMAP handle for windows bitmap.
    WindowsPixmap(isize),
}

impl NativePixmap {
    pub fn raw(&self) -> *mut ffi::c_void {
        match self {
            Self::X11Pixmap(xid) => xid as *const _ as *mut _,
            Self::WindowsPixmap(hbitmap) => hbitmap as *const _ as *mut _,
        }
    }
}

// Handle to the raw OpenGL surface.
#[derive(Debug, Clone, Copy)]
pub enum RawSurface {
    // A pointer to EGLSurface.
    #[cfg(egl_backend)]
    Egl(*const ffi::c_void),

    /// GLXDrawable.
    #[cfg(glx_backend)]
    Glx(u64),

    /// TODO
    #[cfg(wgl_backend)]
    Wgl(*const ffi::c_void),

    /// TODO
    #[cfg(cgl_backend)]
    Cgl(*const ffi::c_void),
}

pub trait AsRawSurface {
    fn raw_surface(&self) -> RawSurface;
}

impl<T: SurfaceTypeTrait> AsRawSurface for Surface<T> {
    fn raw_surface(&self) -> RawSurface {
        dispatch_gl!(self; Self(surface) => surface.raw_surface())
    }
}
