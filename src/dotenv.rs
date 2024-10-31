#[macro_export]
macro_rules! env_var {
    ($name:expr) => {{
        std::env::var($name).expect(concat!("missing env var ", $name))
    }};
}
