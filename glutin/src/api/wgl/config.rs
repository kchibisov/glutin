use std::os::raw::c_int;
use std::sync::Arc;

use crate::config::{
    Api, AsRawConfig, ColorBufferType, ConfigSurfaceTypes, ConfigTemplate, GlConfig, RawConfig,
};
use crate::display::GetGlDisplay;
use crate::private::Sealed;

use super::display::Display;

impl Display {
    pub(crate) fn find_configs(
        &self,
        template: ConfigTemplate,
    ) -> Option<Box<dyn Iterator<Item = Config> + '_>> {
        todo!()
    }
}

#[derive(Clone)]
pub struct Config {
    pub(crate) inner: Arc<ConfigInner>,
}

pub(crate) struct ConfigInner {}

impl Sealed for Config {}

impl GlConfig for Config {
    fn color_buffer_type(&self) -> ColorBufferType {
        todo!()
    }

    fn float_pixels(&self) -> bool {
        todo!()
    }

    fn native_visual(&self) -> u32 {
        todo!()
    }

    fn alpha_size(&self) -> u8 {
        todo!()
    }

    fn srgb_capable(&self) -> bool {
        todo!()
    }

    fn depth_size(&self) -> u8 {
        todo!()
    }

    fn stencil_size(&self) -> u8 {
        todo!()
    }

    fn sample_buffers(&self) -> u8 {
        todo!()
    }

    fn config_surface_types(&self) -> ConfigSurfaceTypes {
        todo!()
    }

    fn api(&self) -> Api {
        todo!()
    }
}

impl GetGlDisplay for Config {
    type Target = Display;
    fn display(&self) -> Self::Target {
        todo!()
    }
}

impl Config {
    fn raw_attribute(&self, attr: c_int) -> c_int {
        todo!()
    }
}

impl AsRawConfig for Config {
    fn raw_config(&self) -> RawConfig {
        todo!()
    }
}
