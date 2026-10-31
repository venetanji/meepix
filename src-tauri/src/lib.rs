#[tauri::command]
fn easel_health() -> String {
    // Stub deliberately performs no network request until the endpoint and
    // access policy are configured through an approved application setting.
    "Easel adapter not configured".into()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![easel_health])
        .run(tauri::generate_context!())
        .expect("error while running MeePix");
}
