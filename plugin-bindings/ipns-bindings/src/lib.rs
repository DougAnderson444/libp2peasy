use extism_pdk::*;
use ipns_plugin_interface::Output;

const VOWELS: &[char] = &['a', 'A', 'e', 'E', 'i', 'I', 'o', 'O', 'u', 'U'];

#[host_fn]
extern "ExtismHost" {
    fn hello_world(count: Json<Output>) -> Json<Output>;
}

#[plugin_fn]
pub fn count_vowels(input: String) -> FnResult<Json<Output>> {
    let mut count = 0;
    for ch in input.chars() {
        if VOWELS.contains(&ch) {
            count += 1;
        }
    }

    set_var!("a", "this is var a!")?;

    let a = var::get("a")?.expect("variable 'a' set");
    let a = String::from_utf8(a).expect("string from varible value");
    let config = config::get("thing").expect("'thing' key set in config");

    let output = Output { count, config, a };
    let output = unsafe { hello_world(Json(output))? };
    Ok(output)
}
