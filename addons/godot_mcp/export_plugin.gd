@tool
extends EditorExportPlugin

## Leaves this addon out of exported games: it is editor-only code (a WebSocket server
## driving the editor) that a game never runs, in debug or release builds.

## Export plugins see each file in the order of their names, and the first one that skips or
## remaps a file ends the chain. The built-in "GDScript" plugin remaps every .gd file to its
## compiled .gdc, so this name must sort before it or the addon's scripts are never offered.
func _get_name() -> String:
    return "ExcludeGodotMcpAddon"

func _export_file(path: String, _type: String, _features: PackedStringArray) -> void:
    # The folder comes from this script's own location, so a moved addon still works.
    var addon_dir: String = (get_script() as Script).resource_path.get_base_dir()
    if should_skip(path, addon_dir):
        skip()

## True for every file inside `addon_dir` and for nothing else. Static so it can be tested
## outside the editor, where an EditorExportPlugin cannot be instantiated.
static func should_skip(path: String, addon_dir: String) -> bool:
    if addon_dir.is_empty():
        return false
    return path.begins_with(addon_dir.trim_suffix("/") + "/")
