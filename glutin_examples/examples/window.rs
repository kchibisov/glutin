use std::ffi::CString;
use std::num::NonZeroU32;

use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
#[cfg(x11_platform)]
use winit::platform::unix::{self, WindowBuilderExtUnix};
use winit::window::WindowBuilder;

use glutin::config::{ConfigSurfaceTypes, ConfigTemplateBuilder};
use glutin::context::ContextAttributesBuilder;
use glutin::display::{Display, DisplayApiPreference, DisplayPicker};
use glutin::prelude::*;
use glutin::surface::{SurfaceAttributesBuilder, WindowSurface};

mod gl {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

fn main() {
    let event_loop = EventLoop::new();
    let raw_display = event_loop.raw_display_handle();

    let window = WindowBuilder::new().with_transparent(true).build(&event_loop).unwrap();
    let raw_window_handle = window.raw_window_handle();

    #[cfg(not(all(egl_backend, glx_backend, wgl_backend)))]
    let picker = DisplayPicker::new();
    #[cfg(all(egl_backend, wgl_backend))]
    let picker = DisplayPicker::new()
        .with_most_compatible_for_window(raw_window_handle)
        .with_api_preference(DisplayApiPreference::WglThenEgl);
    #[cfg(all(egl_backend, glx_backend))]
    let picker = DisplayPicker::new()
        .with_api_preference(DisplayApiPreference::GlxThenEgl)
        .with_glx_error_registrator(Box::new(unix::register_xlib_error_hook));

    // Create connection to underlying OpenGL client Api.
    let gl_display = unsafe { Display::from_raw(raw_display, picker).unwrap() };

    // Create template to find OpenGL config.
    let config_template = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .with_transparency(true)
        .compatible_with_native_window(raw_window_handle)
        .with_surface_type(ConfigSurfaceTypes::WINDOW)
        .build();

    let gl_config = &gl_display.find_configs(config_template).unwrap().next().unwrap();

    window.set_visible(true);

    let (width, height): (u32, u32) = window.inner_size().into();

    let surface_attributes = SurfaceAttributesBuilder::<WindowSurface>::new().build(
        raw_window_handle,
        NonZeroU32::new(width).unwrap(),
        NonZeroU32::new(height).unwrap(),
    );
    let gl_surface =
        unsafe { gl_display.create_window_surface(gl_config, &surface_attributes).unwrap() };

    let context_attributes = ContextAttributesBuilder::new().build_windowed(raw_window_handle);
    let gl_context = gl_display.create_context(&gl_config, &context_attributes).unwrap();

    let gl_context = gl_context.make_current(&gl_surface).unwrap();

    gl::load_with(|symbol| {
        let symbol = CString::new(symbol).unwrap();
        gl_context.get_proc_address(symbol.as_c_str()) as *const _
    });

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(size) => {
                    if size.width != 0 && size.height != 0 {
                        gl_surface.resize(
                            &gl_context,
                            NonZeroU32::new(size.width).unwrap(),
                            NonZeroU32::new(size.height).unwrap(),
                        );
                    }
                }
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                _ => (),
            },
            Event::RedrawEventsCleared => {
                unsafe {
                    gl::ClearColor(1., 0., 1., 0.5);
                    gl::Clear(gl::COLOR_BUFFER_BIT);
                    window.request_redraw();
                }

                let _ = gl_surface.swap_buffers(&gl_context);
            }
            _ => (),
        }
    });
}
