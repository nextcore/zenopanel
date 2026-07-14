pub mod common;
pub mod image;
pub mod container;
pub mod volume;
pub mod network;
pub mod compose;

pub use common::get_runc_bin;
pub use container::container_list_internal;

use zenocore::Engine;

pub fn register(engine: &mut Engine) {
    image::register(engine);
    container::register(engine);
    volume::register(engine);
    network::register(engine);
    compose::register(engine);
}
