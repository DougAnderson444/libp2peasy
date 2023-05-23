//! # Libp2peasy Plugins
//!
//! This crate contains the plugin system for libp2peasy. It is not intended to be used directly.
//!
//! This create defines the Rust Traits that plugins must implement in order to be used by libp2peasy.
//!
//! ## Usage
//!
//! ```rust
//! use libp2peasy_plugins::Plugin;
//!
//! struct MyPlugin;
//!
//! impl Plugin for MyPlugin {
//!    fn name(&self) -> &'static str {
//!       "my-plugin"
//!   }
//! }
//!

/// The Plugin trait defines the methods that must be implemented by a plugin.
// pub trait Plugin {
//     /// Returns the name of the plugin.
//     fn name(&self) -> &'static str;
// }

#[cfg(test)]
mod tests {
    use anyhow::{Error, Result};
    use extism::Context;
    use extism::Function;
    use extism::Plugin;
    use extism::ValType;
    use extism::{CurrentPlugin, UserData, Val};
    use vowels_interface::Output;

    /// Host function that is called by the plugin
    // https://github.com/extism/rust-pdk/blob/main/examples/host_function.rs
    pub fn hello_world(
        _plugin: &mut CurrentPlugin,
        inputs: &[Val],
        outputs: &mut [Val],
        _user_data: UserData,
    ) -> Result<(), Error> {
        println!("Hello from Rust Host Function!");
        outputs[0] = inputs[0].clone();
        Ok(())
    }

    #[test]
    fn it_works() -> Result<()> {
        let wasm = include_bytes!(
            "../../target/wasm32-unknown-unknown/release/libp2peasy_plugin_ipns_bindings.wasm"
        );

        let context = Context::new();
        let f = Function::new(
            "hello_world",
            [ValType::I64],
            [ValType::I64],
            None,
            hello_world,
        );
        let thing = "this".to_string();
        let mut config = std::collections::BTreeMap::new();
        config.insert("thing".to_string(), Some(thing.to_owned()));

        let wasi = false;
        let mut plugin = Plugin::new(&context, wasm, [f], wasi)?.with_config(&config)?;

        let data = plugin.call("count_vowels", "this is a test").unwrap();

        // convert bytes to string
        let data_str = std::str::from_utf8(data).unwrap();
        eprintln!("{data_str:?}");

        let d: Output = serde_json::from_str(data_str).unwrap();

        assert_eq!(d.count, 4);
        assert_eq!(d.config, thing);
        assert_eq!(d.a, "this is var a!");

        Ok(())
    }
}
