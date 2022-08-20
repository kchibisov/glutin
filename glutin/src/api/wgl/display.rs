use std::collections::HashSet;
use std::ffi::{CStr, CString, OsStr};
use std::mem;
use std::ops::{Deref, DerefMut};
use std::os::windows::ffi::OsStrExt;
use std::sync::Arc;

use glutin_wgl_sys::wgl;
use glutin_wgl_sys::wgl_extra;
use once_cell::sync::OnceCell;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::{HINSTANCE, HWND};
use windows_sys::Win32::Graphics::Gdi::{self as gdi, HDC};
use windows_sys::Win32::Graphics::OpenGL::{self as gl, PIXELFORMATDESCRIPTOR};
use windows_sys::Win32::System::LibraryLoader as dll_loader;
use windows_sys::Win32::UI::WindowsAndMessaging::{self as wm, WINDOWPLACEMENT, WNDCLASSEXW};

use crate::config::ConfigTemplate;
use crate::display::{AsRawDisplay, RawDisplay};
use crate::error::{ErrorKind, Result};
use crate::prelude::*;
use crate::private::Sealed;
use crate::surface::{PbufferSurface, PixmapSurface, SurfaceAttributes, WindowSurface};

use super::config::{self, Config};
use super::context::NotCurrentContext;
use super::surface::Surface;

#[derive(Clone)]
pub struct Display {
    pub(crate) inner: Arc<DisplayInner>,
}

pub(crate) struct DisplayInner {
    /// Client WGL extensions.
    pub(crate) lib_opengl32: HINSTANCE,

    /// Extra functions used by the impl.
    pub(crate) wgl_extra: Option<&'static WglExtra>,

    pub(crate) client_extensions: HashSet<&'static str>,
}

impl Display {
    /// Create WGL display.
    pub unsafe fn from_raw(
        display: RawDisplayHandle,
        native_window: Option<RawWindowHandle>,
    ) -> Result<Self> {
        if !matches!(display, RawDisplayHandle::Windows(..)) {
            return Err(ErrorKind::NotSupported.into());
        }

        let name =
            OsStr::new("opengl32.dll").encode_wide().chain(Some(0).into_iter()).collect::<Vec<_>>();
        let lib_opengl32 = dll_loader::LoadLibraryW(name.as_ptr());
        if lib_opengl32 == 0 {
            return Err(ErrorKind::NotFound.into());
        }

        // In case native window was provided init extra functions.
        let (wgl_extra, client_extensions) =
            if let Some(RawWindowHandle::Win32(window)) = native_window {
                let (wgl_extra, client_extensions) = load_extra_functions(window.hwnd as _)?;
                println!("EXT: {:?}", client_extensions);
                (Some(wgl_extra), client_extensions)
            } else {
                (None, HashSet::new())
            };

        let inner = Arc::new(DisplayInner { lib_opengl32, wgl_extra, client_extensions });
        Ok(Display { inner })
    }
}

fn load_extensions(hdc: HDC, wgl_extra: &WglExtra) -> HashSet<&'static str> {
    let extensions = unsafe {
        if wgl_extra.GetExtensionsStringARB.is_loaded() {
            CStr::from_ptr(wgl_extra.GetExtensionsStringARB(hdc as *const _))
        } else if wgl_extra.GetExtensionsStringEXT.is_loaded() {
            CStr::from_ptr(wgl_extra.GetExtensionsStringEXT())
        } else {
            return HashSet::new();
        }
    };

    if let Ok(extensions) = extensions.to_str() {
        extensions.split(' ').collect::<HashSet<_>>()
    } else {
        HashSet::new()
    }
}

unsafe fn load_extra_functions(win: HWND) -> Result<(&'static WglExtra, HashSet<&'static str>)> {
    let mut placement: WINDOWPLACEMENT = std::mem::zeroed();
    placement.length = mem::size_of::<WINDOWPLACEMENT>() as _;
    if wm::GetWindowPlacement(win, &mut placement) == 0 {
        return Err(ErrorKind::BadNativeWindow.into());
    }
    let rect = placement.rcNormalPosition;

    let mut class_name = [0u16; 128];
    if wm::GetClassNameW(win, class_name.as_mut_ptr(), 128) == 0 {
        return Err(ErrorKind::BadNativeWindow.into());
    }

    let instance = dll_loader::GetModuleHandleW(std::ptr::null());
    let mut class: WNDCLASSEXW = std::mem::zeroed();
    if wm::GetClassInfoExW(instance, class_name.as_ptr(), &mut class) == 0 {
        return Err(ErrorKind::BadNativeWindow.into());
    }

    let class_name =
        OsStr::new("WglDummy Window").encode_wide().chain(Some(0).into_iter()).collect::<Vec<_>>();

    class.cbSize = mem::size_of::<WNDCLASSEXW>() as _;
    class.lpszClassName = class_name.as_ptr();
    class.lpfnWndProc = Some(wm::DefWindowProcW);

    // This shouldn't fail if the registration of the real window class
    // worked. Multiple registrations of the window class trigger an
    // error which we want to ignore silently (e.g for multi-window
    // setups).
    wm::RegisterClassExW(&class);

    // This dummy wnidow should match the real one enough to get the same OpenGL driver.
    let title =
        OsStr::new("dummy window").encode_wide().chain(Some(0).into_iter()).collect::<Vec<_>>();

    let ex_style = wm::WS_EX_APPWINDOW;
    let style = wm::WS_POPUP | wm::WS_CLIPSIBLINGS | wm::WS_CLIPCHILDREN;
    let win = wm::CreateWindowExW(
        ex_style,
        class_name.as_ptr(),
        title.as_ptr() as _,
        style,
        wm::CW_USEDEFAULT,
        wm::CW_USEDEFAULT,
        rect.right - rect.left,
        rect.bottom - rect.top,
        0,
        0,
        instance,
        std::ptr::null_mut(),
    );

    if win == 0 {
        return Err(ErrorKind::OutOfMemory.into());
    }

    let hdc = gdi::GetDC(win);
    let (pixel_format_index, descriptor) = config::choose_dummy_pixel_format(hdc)?;
    if gl::SetPixelFormat(hdc, pixel_format_index, &descriptor) == 0 {
        return Err(ErrorKind::BadConfig.into());
    }

    let context = gl::wglCreateContext(hdc);
    gl::wglMakeCurrent(hdc, context);

    // Load WGL.
    let wgl_extra = WGL_EXTRA.get_or_init(|| WglExtra::new());
    let client_extensions = load_extensions(hdc, wgl_extra);

    wm::DestroyWindow(win);
    gl::wglDeleteContext(context);

    Ok((wgl_extra, client_extensions))
}

pub(crate) static WGL_EXTRA: OnceCell<WglExtra> = OnceCell::new();

pub(crate) struct WglExtra(wgl_extra::Wgl);

unsafe impl Send for WglExtra {}
unsafe impl Sync for WglExtra {}

impl Deref for WglExtra {
    type Target = wgl_extra::Wgl;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl WglExtra {
    fn new() -> Self {
        Self(wgl_extra::Wgl::load_with(|addr| unsafe {
            let addr = CString::new(addr.as_bytes()).unwrap();
            let addr = addr.as_ptr();
            wgl::GetProcAddress(addr).cast()
        }))
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
        RawDisplay::Wgl
    }
}
