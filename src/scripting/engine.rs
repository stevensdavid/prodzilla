use rhai::Engine;

pub fn create_engine() -> Engine {
    let mut engine = Engine::new();
    engine.set_max_operations(1_000_000);
    engine.set_max_call_levels(64);
    engine.set_max_string_size(1_048_576); // 1MB
    engine.set_max_array_size(10_000);
    engine.set_max_map_size(10_000);
    engine
}
