use std::iter;
use std::sync::Arc;

use cocoa::appkit::{NSOpenGLPixelFormat, NSOpenGLPixelFormatAttribute};
use cocoa::base::{id, nil};

use crate::config::{
    Api, AsRawConfig, ColorBufferType, ConfigSurfaceTypes, ConfigTemplate, GlConfig, RawConfig,
};
use crate::display::GetGlDisplay;
use crate::error::{ErrorKind, Result};
use crate::private::Sealed;

use super::display::Display;

impl Display {
    pub(crate) fn find_configs(
        &self,
        template: ConfigTemplate,
    ) -> Result<Box<dyn Iterator<Item = Config> + '_>> {
        let mut attrs = Vec::<u32>::with_capacity(32);

        // We use minimum to follow behavior of other platforms here.
        attrs.push(NSOpenGLPixelFormatAttribute::NSOpenGLPFAMinimumPolicy as u32);

        // Color.
        match template.color_buffer_type {
            ColorBufferType::Rgb { r_size, g_size, b_size } => {
                attrs.push(NSOpenGLPixelFormatAttribute::NSOpenGLPFAColorSize as u32);
                // We can't specify particular color, so we provide the sum.
                attrs.push((r_size + g_size + b_size) as u32);
            }
            _ => return Err(ErrorKind::NotSupported.into()),
        }

        // Alpha.
        attrs.push(NSOpenGLPixelFormatAttribute::NSOpenGLPFAAlphaSize as u32);
        attrs.push(template.alpha_size as u32);

        // Depth.
        attrs.push(NSOpenGLPixelFormatAttribute::NSOpenGLPFADepthSize as u32);
        attrs.push(template.depth_size as u32);

        // Stencil.
        attrs.push(NSOpenGLPixelFormatAttribute::NSOpenGLPFAStencilSize as u32);
        attrs.push(template.stencil_size as u32);

        // Float colors.
        if template.float_pixels {
            attrs.push(NSOpenGLPixelFormatAttribute::NSOpenGLPFAColorFloat as u32);
        }

        // Sample buffers.
        if template.sample_buffers != 0 {
            attrs.push(NSOpenGLPixelFormatAttribute::NSOpenGLPFAMultisample as u32);
            attrs.push(NSOpenGLPixelFormatAttribute::NSOpenGLPFASampleBuffers as u32);
            attrs.push(template.sample_buffers as u32);
        }

        // Double buffering.
        if !template.single_buffering {
            attrs.push(NSOpenGLPixelFormatAttribute::NSOpenGLPFADoubleBuffer as u32);
        }

        // Stereo.
        if template.stereoscopy == Some(true) {
            attrs.push(NSOpenGLPixelFormatAttribute::NSOpenGLPFAStereo as u32);
        }

        // TODO profile?

        // Attrs are zero terminated.
        attrs.push(0 as u32);

        unsafe {
            let raw = NSOpenGLPixelFormat::alloc(nil).initWithAttributes_(&attrs);
            if raw.is_null() {
                return Err(ErrorKind::BadConfig.into());
            }
            let inner = Arc::new(ConfigInner { raw, transrarency: template.transparency });
            let config = Config { inner };

            Ok(Box::new(iter::once(config).into_iter()))
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub(crate) inner: Arc<ConfigInner>,
}

pub(crate) struct ConfigInner {
    pub(crate) transrarency: bool,
    pub(crate) raw: id,
}

impl Drop for ConfigInner {
    fn drop(&mut self) {
        if self.raw != nil {
            let _: () = unsafe { msg_send![self.raw, release] };
        }
    }
}

impl Sealed for Config {}

impl GlConfig for Config {
    fn color_buffer_type(&self) -> ColorBufferType {
        // On macos all color formats divide by 3 without reminder, except for the RGB 565. So we
        // can convert it in a hopefully reliable way.
        let color = self.raw_attribute(NSOpenGLPixelFormatAttribute::NSOpenGLPFAColorSize);
        let r_size = (color / 3) as u8;
        let b_size = (color / 3) as u8;
        let g_size = (color - r_size as i32 - b_size as i32) as u8;
        ColorBufferType::Rgb { r_size, g_size, b_size }
    }

    fn float_pixels(&self) -> bool {
        self.raw_attribute(NSOpenGLPixelFormatAttribute::NSOpenGLPFAColorFloat) != 0
    }

    fn native_visual(&self) -> u32 {
        todo!()
    }

    fn alpha_size(&self) -> u8 {
        self.raw_attribute(NSOpenGLPixelFormatAttribute::NSOpenGLPFAAlphaSize) as u8
    }

    fn srgb_capable(&self) -> bool {
        true
    }

    fn depth_size(&self) -> u8 {
        self.raw_attribute(NSOpenGLPixelFormatAttribute::NSOpenGLPFADepthSize) as u8
    }

    fn stencil_size(&self) -> u8 {
        self.raw_attribute(NSOpenGLPixelFormatAttribute::NSOpenGLPFAStencilSize) as u8
    }

    fn sample_buffers(&self) -> u8 {
        self.raw_attribute(NSOpenGLPixelFormatAttribute::NSOpenGLPFASampleBuffers) as u8
    }

    fn config_surface_types(&self) -> ConfigSurfaceTypes {
        ConfigSurfaceTypes::WINDOW
    }

    fn api(&self) -> Api {
        Api::OPENGL
    }
}

impl Config {
    fn raw_attribute(&self, attrib: NSOpenGLPixelFormatAttribute) -> i32 {
        unsafe {
            let mut value = 0;
            NSOpenGLPixelFormat::getValues_forAttribute_forVirtualScreen_(
                self.inner.raw,
                &mut value,
                attrib,
                // They do differ per monitor and require context. Which is kind of insane, but
                // whatever. Zero is a primary monitor.
                0,
            );
            value as i32
        }
    }
}

impl GetGlDisplay for Config {
    type Target = Display;
    fn display(&self) -> Self::Target {
        Display
    }
}

impl AsRawConfig for Config {
    fn raw_config(&self) -> RawConfig {
        RawConfig::Cgl(self.inner.raw.cast())
    }
}
