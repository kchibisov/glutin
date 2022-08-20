//! A cross platform OpenGL platform api library.

#[cfg(all(not(egl_backend), not(glx_backend), not(wgl_backend), not(cgl_backend)))]
compile_error!("Please select at least one api backend");

pub mod api;
pub mod config;
pub mod context;
pub mod display;
pub mod error;
pub mod platform;
pub mod prelude;
pub mod surface;

#[cfg(any(egl_backend, glx_backend, wgl_backend))]
mod lib_loading;

#[cfg(cgl_backend)]
#[macro_use]
extern crate objc;

pub(crate) mod gl_dipsatch {
    /// `dispatch_gl!(match expr; Enum(foo) => foo.something())`
    /// expands to the equivalent of
    /// ```ignore
    /// match self {
    ///    Enum::Egl(foo) => foo.something(),
    ///    Enum::Glx(foo) => foo.something(),
    ///    Enum::Wgl(foo) => foo.something(),
    ///    Enum::Cgl(foo) => foo.something(),
    /// }
    /// ```
    /// The result can be converted to another enum by adding `; as AnotherEnum`
    #[macro_export]
    macro_rules! dispatch_gl {
        ($what:ident; $enum:ident ( $($c1:tt)* ) => $x:expr; as $enum2:ident ) => {
            match $what {
                #[cfg(egl_backend)]
                $enum::Egl($($c1)*) => $enum2::Egl($x),
                #[cfg(glx_backend)]
                $enum::Glx($($c1)*) => $enum2::Glx($x),
                #[cfg(wgl_backend)]
                $enum::Wgl($($c1)*) => $enum2::Wgl($x),
                #[cfg(cgl_backend)]
                $enum::Cgl($($c1)*) => $enum2::Cgl($x),
            }
        };
        ($what:ident; $enum:ident ( $($c1:tt)* ) => $x:expr) => {
            match $what {
                #[cfg(egl_backend)]
                $enum::Egl($($c1)*) => $x,
                #[cfg(glx_backend)]
                $enum::Glx($($c1)*) => $x,
                #[cfg(wgl_backend)]
                $enum::Wgl($($c1)*) => $x,
                #[cfg(cgl_backend)]
                $enum::Cgl($($c1)*) => $x,
            }
        };
    }
}

mod private {
    /// Prevent traits from being implemented downstream.
    pub trait Sealed {}
}
