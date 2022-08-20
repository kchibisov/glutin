//! Dedicated utils to work with X11 shenanigans.

use std::mem;

use once_cell::sync::Lazy;
use x11_dl::xlib::{Display, XVisualInfo, Xlib};
#[cfg(egl_backend)]
use x11_dl::xlib::{VisualIDMask, XID};
use x11_dl::xrender::Xrender;

pub(crate) static XLIB: Lazy<Option<Xlib>> = Lazy::new(|| Xlib::open().ok());
static XRENDER: Lazy<Option<Xrender>> = Lazy::new(|| Xrender::open().ok());

pub trait X11GlConfigExt {
    // The `X11VisualInfo` that must be used to inititalize the Xlib window.
    fn x11_visual(&self) -> Option<X11VisualInfo>;
}

/// The X11 visual info.
///
/// This must be used when building X11 window, so it'll be compatible with the underlying Api.
pub struct X11VisualInfo {
    // FIXME don't store display.
    display: *mut Display,
    raw: *const XVisualInfo,
}

impl X11VisualInfo {
    #[cfg(egl_backend)]
    pub(crate) unsafe fn from_xid(display: *mut Display, xid: XID) -> Option<Self> {
        let xlib = XLIB.as_ref().unwrap();

        if xid == 0 {
            return None;
        }

        let mut raw: XVisualInfo = std::mem::zeroed();
        raw.visualid = xid;

        let mut num_visuals = 0;
        let raw = (xlib.XGetVisualInfo)(display, VisualIDMask, &mut raw, &mut num_visuals);

        if raw.is_null() {
            return None;
        }

        Some(Self { display, raw })
    }

    pub(crate) unsafe fn from_raw(display: *mut Display, raw: *const XVisualInfo) -> Self {
        Self { display, raw }
    }

    /// Check wether the [`Self`] supports transparency.
    pub fn supports_transparency(&self) -> bool {
        let xrender = XRENDER.as_ref().unwrap();
        unsafe {
            let visual_format = (xrender.XRenderFindVisualFormat)(self.display, (*self.raw).visual);

            (!visual_format.is_null())
                .then(|| (*visual_format).direct.alphaMask != 0)
                .unwrap_or(false)
        }
    }

    /// Convert the visual to the raw pointer.
    ///
    /// You must clear it with `XFree` after the use.
    pub fn into_raw(self) -> *const std::ffi::c_void {
        let raw = self.raw as *const _;
        mem::forget(self);
        raw
    }
}

impl Drop for X11VisualInfo {
    fn drop(&mut self) {
        unsafe {
            (XLIB.as_ref().unwrap().XFree)(self.raw as *mut _);
        }
    }
}
