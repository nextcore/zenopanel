use zenocore::Engine;

pub fn register(engine: &mut Engine) {
    crate::proxyman::register_proxy_slots(engine);
}
