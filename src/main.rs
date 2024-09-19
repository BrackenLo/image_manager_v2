//====================================================================

use debug::DebugPlugin;
use images::ImagePlugin;
use layout::LayoutPlugin;
use renderer::CustomRendererPlugin;
use shipyard_renderer::RendererPlugin;
use shipyard_runner::{tools::ToolsPlugin, AppBuilder, DefaultInner, Runner};
use storage::StoragePlugin;

pub(crate) mod debug;
pub(crate) mod images;
pub(crate) mod layout;
pub(crate) mod renderer;
pub(crate) mod storage;
pub(crate) mod tools;

//====================================================================

const NAME: &str = env!("CARGO_PKG_NAME");

fn main() {
    println!("Hello, world!");

    env_logger::Builder::new()
        .filter_module("wgpu", log::LevelFilter::Warn)
        .filter_module("shipyard_tools", log::LevelFilter::Trace)
        .filter_module("shipyard_renderer", log::LevelFilter::Trace)
        .filter_module("shipyard_shared", log::LevelFilter::Trace)
        .filter_module("shipyard_runner", log::LevelFilter::Trace)
        .filter_module(NAME, log::LevelFilter::Trace)
        .format_timestamp(None)
        .init();

    Runner::<DefaultInner<App>>::new().run();
}

//====================================================================

pub struct App;
impl AppBuilder for App {
    fn build(builder: shipyard_tools::WorkloadBuilder) -> shipyard_tools::WorkloadBuilder {
        builder
            .add_plugin(ToolsPlugin)
            .add_plugin(RendererPlugin)
            .add_plugin(CustomRendererPlugin)
            .add_plugin(DebugPlugin)
            .add_plugin(StoragePlugin)
            .add_plugin(LayoutPlugin)
            .add_plugin(ImagePlugin)
    }
}

//====================================================================
