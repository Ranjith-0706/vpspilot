use serde::Serialize;

#[derive(Serialize)]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
}

/// Read-only application metadata.
#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        name: "VPSPilot",
        version: env!("CARGO_PKG_VERSION"),
    }
}
