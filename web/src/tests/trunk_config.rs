//! trunk 0.22+ downloads Tailwind v4 unless told otherwise. This project uses the Tailwind v3
//! setup (tailwind.config.js), which v4 turns into an incomplete stylesheet: the sidebar drawer
//! of the 0.9.2 desktop client opened out of view (upstream #1091). Keep the pin on v3.

use wasm_bindgen_test::wasm_bindgen_test;

const TRUNK_TOML: &str = include_str!("../../Trunk.toml");

/// The value of `tailwindcss = "..."` in the `[tools]` table.
fn tailwind_pin(toml: &str) -> Option<&str> {
    let mut in_tools = false;
    for line in toml.lines().map(str::trim) {
        if line.starts_with('[') {
            in_tools = line == "[tools]";
            continue;
        }
        if in_tools {
            if let Some(value) = line.strip_prefix("tailwindcss").map(str::trim_start).and_then(|r| r.strip_prefix('=')) {
                return Some(value.trim().trim_matches('"'));
            }
        }
    }
    None
}

#[wasm_bindgen_test]
fn trunk_pins_tailwind_v3() {
    let pin = tailwind_pin(TRUNK_TOML).expect("web/Trunk.toml [tools] must pin tailwindcss");
    assert!(pin.starts_with("3."), "web/Trunk.toml pins tailwindcss = {pin:?}; the project needs Tailwind v3");
}

#[wasm_bindgen_test]
fn tailwind_pin_is_read_from_the_tools_table_only() {
    assert_eq!(tailwind_pin("[tools]\nwasm-opt = [\"-Oz\"]\ntailwindcss = \"3.3.5\"\n"), Some("3.3.5"));
    assert_eq!(tailwind_pin("[serve]\ntailwindcss = \"3.3.5\"\n[tools]\n"), None);
    assert_eq!(tailwind_pin("[tools]\nwasm-opt = [\"-Oz\"]\n"), None);
}
