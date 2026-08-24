mod discovery;
mod pairing;
mod session;
mod sidecar;

use discovery::DiscoverySnapshot;
use session::SessionProcess;
use sidecar::{HostProcess, SidecarStatus};
use tauri::{Manager, State};

#[tauri::command]
fn sidecar_status(host: State<HostProcess>) -> SidecarStatus {
    SidecarStatus::snapshot(&host)
}

#[tauri::command]
fn list_peers() -> DiscoverySnapshot {
    discovery::list_peers()
}

#[tauri::command]
async fn connect(app: tauri::AppHandle, ip: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<SessionProcess>().connect(&ip))
        .await
        .map_err(|err| format!("Connect task failed: {err}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(HostProcess::new())
        .manage(SessionProcess::new())
        .invoke_handler(tauri::generate_handler![sidecar_status, list_peers, connect])
        .setup(|app| {
            app.state::<HostProcess>().start_sunshine();
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Eclipse")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<SessionProcess>().stop();
                app.state::<HostProcess>().stop_sunshine();
            }
        });
}
