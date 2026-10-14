#[test]
fn test_plugin_cfg_version_matches_cargo() {
    let plugin_cfg = include_str!("../addons/godot_mcp/plugin.cfg");
    let plugin_version = plugin_cfg
        .lines()
        .find_map(|line| line.trim().strip_prefix("version="))
        .map(|value| value.trim_matches('"'))
        .expect("plugin.cfg has no version= entry");

    assert_eq!(
        plugin_version,
        env!("CARGO_PKG_VERSION"),
        "addons/godot_mcp/plugin.cfg version must match Cargo.toml; bump both together"
    );
}
