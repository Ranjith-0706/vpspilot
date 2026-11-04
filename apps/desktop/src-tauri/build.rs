// Every app command must be listed here; each gets an `allow-<command>` permission that the
// capability file must grant explicitly. Keep this in sync with `generate_handler!` in lib.rs.
const COMMANDS: &[&str] = &[
    "get_app_info",
    "list_servers",
    "add_server",
    "update_server",
    "delete_server",
    "set_server_credential",
    "clear_server_credential",
    "test_server_connection",
    "trust_host_key",
    "forget_host_key",
    "list_host_keys",
    "open_terminal",
    "terminal_write",
    "terminal_resize",
    "close_terminal",
    "start_monitoring",
    "stop_monitoring",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
