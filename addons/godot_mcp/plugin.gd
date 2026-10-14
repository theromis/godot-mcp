@tool
extends EditorPlugin

const MIN_MAJOR: int = 4
const MIN_MINOR: int = 5
const BRIDGE_SCRIPT_PATH: String = "res://addons/godot_mcp/bridge_server.gd"

var bridge_node: Node = null

func _enter_tree() -> void:
    var version: Dictionary = Engine.get_version_info()
    var major: int = version["major"]
    var minor: int = version["minor"]
    if major < MIN_MAJOR or (major == MIN_MAJOR and minor < MIN_MINOR):
        printerr("[Godot MCP] Requires Godot %d.%d or newer, but this is Godot %s. The bridge was not started." % [MIN_MAJOR, MIN_MINOR, version["string"]])
        return

    # Loaded at runtime (not through its class_name) so that a script which fails to compile
    # reports a clear message instead of breaking this plugin as well.
    var bridge_script: GDScript = load(BRIDGE_SCRIPT_PATH)
    if bridge_script == null or not bridge_script.can_instantiate():
        printerr("[Godot MCP] Failed to load ", BRIDGE_SCRIPT_PATH, ". Check the Output panel for script errors.")
        return

    print("[Godot MCP] Initializing Godot MCP plugin...")
    bridge_node = bridge_script.new()
    bridge_node.name = "GodotMcpBridgeServer"
    bridge_node.set("undo_redo", get_undo_redo())
    add_child(bridge_node)
    print("[Godot MCP] Plugin activated successfully")

func _exit_tree() -> void:
    if bridge_node != null:
        bridge_node.call("stop_server")
        bridge_node.queue_free()
        bridge_node = null
        print("[Godot MCP] Plugin deactivated")
