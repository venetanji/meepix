use meepix_app::{App, Generation, Models};
use sprite_core::{EditCommand, Snapshot};
use std::sync::Arc;
use tauri::{Manager, State};

type Native = Arc<App>;
async fn worker<T: Send + 'static>(
    state: State<'_, Native>,
    action: impl FnOnce(&App) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let app = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || action(&app))
        .await
        .map_err(|_| "Native worker failed".to_string())?
}
#[tauri::command]
async fn document_snapshot(state: State<'_, Native>) -> Result<Snapshot, String> {
    worker(state, |app| app.snapshot()).await
}
#[tauri::command]
async fn document_create(
    state: State<'_, Native>,
    width: u32,
    height: u32,
) -> Result<Snapshot, String> {
    worker(state, move |app| app.create(width, height)).await
}
#[tauri::command]
async fn document_command(
    state: State<'_, Native>,
    expected_revision: u64,
    command: EditCommand,
) -> Result<Snapshot, String> {
    worker(state, move |app| app.command(expected_revision, command)).await
}
#[tauri::command]
async fn frame_snapshot(state: State<'_, Native>, frame_id: String) -> Result<Vec<u8>, String> {
    worker(state, move |app| app.frame(&frame_id)).await
}
#[tauri::command]
async fn project_save(state: State<'_, Native>, name: String) -> Result<Snapshot, String> {
    worker(state, move |app| app.save(&name)).await
}
#[tauri::command]
async fn project_load(state: State<'_, Native>, name: String) -> Result<Snapshot, String> {
    worker(state, move |app| app.load(&name)).await
}
#[tauri::command]
async fn project_export(state: State<'_, Native>, name: String) -> Result<String, String> {
    worker(state, move |app| app.export(&name)).await
}
#[tauri::command]
async fn easel_models(state: State<'_, Native>) -> Result<Models, String> {
    worker(state, |app| app.models()).await
}
#[tauri::command]
async fn generation_submit(
    state: State<'_, Native>,
    expected_revision: u64,
    model: String,
    prompt: String,
    mode: String,
) -> Result<Generation, String> {
    worker(state, move |app| {
        app.submit(expected_revision, &model, &prompt, &mode)
    })
    .await
}
#[tauri::command]
async fn generation_poll(state: State<'_, Native>, id: String) -> Result<Generation, String> {
    worker(state, move |app| app.poll(&id)).await
}
#[tauri::command]
async fn generation_reject(state: State<'_, Native>, id: String) -> Result<(), String> {
    worker(state, move |app| app.reject(&id)).await
}
#[tauri::command]
async fn generation_accept(
    state: State<'_, Native>,
    id: String,
    expected_revision: u64,
) -> Result<Snapshot, String> {
    worker(state, move |app| app.accept(&id, expected_revision)).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Environment credentials remain exclusively in Rust. Malformed configuration
            // disables generation without preventing local editing or project recovery.
            let url = std::env::var("EASEL_URL").ok();
            let token = std::env::var("EASEL_API_KEY").ok();
            let client = if url.is_some() || token.is_some() {
                easel_client::Client::new(
                    url.as_deref().unwrap_or("https://easel.ait4x.org"),
                    token,
                )
                .ok()
            } else {
                None
            };
            let storage = app.path().app_data_dir()?;
            app.manage(Arc::new(
                App::new(storage, client).map_err(std::io::Error::other)?,
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            document_snapshot,
            document_create,
            document_command,
            frame_snapshot,
            project_save,
            project_load,
            project_export,
            easel_models,
            generation_submit,
            generation_poll,
            generation_reject,
            generation_accept
        ])
        .run(tauri::generate_context!())
        .expect("error while running MeePix");
}
