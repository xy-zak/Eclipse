mod discovery;
mod display;
mod pairing;
mod session;
mod sidecar;

use discovery::DiscoverySnapshot;
use pairing::InboundStream;
use session::{SessionProcess, SessionStatus, StreamPrefs};
use sidecar::{HostProcess, SidecarStatus};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State, WindowEvent};

#[tauri::command]
fn sidecar_status(host: State<HostProcess>) -> SidecarStatus {
    SidecarStatus::snapshot(&host)
}

#[tauri::command]
fn list_peers() -> DiscoverySnapshot {
    discovery::list_peers()
}

#[tauri::command]
fn session_status(session: State<SessionProcess>) -> SessionStatus {
    session.status()
}

#[tauri::command]
fn inbound_status() -> Option<InboundStream> {
    pairing::local_inbound()
}

#[tauri::command]
fn force_disconnect_inbound() -> Result<(), String> {
    pairing::force_disconnect_inbound()
}

#[tauri::command]
fn disconnect(session: State<SessionProcess>) {
    session.stop();
}

#[tauri::command]
fn peer_latency(ip: String) -> Option<u32> {
    discovery::peer_latency_ms(&ip)
}

#[tauri::command]
async fn connect(app: tauri::AppHandle, ip: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<SessionProcess>().connect(&ip))
        .await
        .map_err(|err| format!("Connect task failed: {err}"))?
}

#[tauri::command]
async fn set_stream_prefs(app: tauri::AppHandle, prefs: StreamPrefs) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<SessionProcess>().set_prefs(prefs))
        .await
        .map_err(|err| format!("Prefs task failed: {err}"))?
}

#[tauri::command]
async fn open_all_displays(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<SessionProcess>().open_all_displays())
        .await
        .map_err(|err| format!("Open-all task failed: {err}"))?
}

fn start_hidden() -> bool {
    std::env::args().any(|a| a == "--tray")
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn host_tooltip(on: bool) -> String {
    if on {
        "Eclipse — host streaming on".into()
    } else {
        "Eclipse — host streaming off".into()
    }
}

fn sync_host_menu(app: &AppHandle, host_item: &CheckMenuItem<tauri::Wry>) {
    let on = app.state::<HostProcess>().sunshine_running();
    let _ = host_item.set_checked(on);
    if let Some(tray) = app.tray_by_id("eclipse") {
        let _ = tray.set_tooltip(Some(host_tooltip(on)));
    }
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let host_on = app.state::<HostProcess>().sunshine_running();
    let open = MenuItem::with_id(app, "open", "Open Eclipse", true, None::<&str>)?;
    let host_item =
        CheckMenuItem::with_id(app, "host", "Host streaming", true, host_on, None::<&str>)?;
    let host_for_menu = host_item.clone();
    let host_for_tray = host_item.clone();
    let quit = MenuItem::with_id(app, "quit", "Quit Eclipse", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &host_item,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id("eclipse")
        .icon(
            app.default_window_icon()
                .cloned()
                .expect("Eclipse window icon"),
        )
        .tooltip(host_tooltip(host_on))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "open" => show_main_window(app),
            "host" => {
                let proc = app.state::<HostProcess>();
                let want = !proc.sunshine_running();
                if let Err(err) = proc.set_host_running(want) {
                    eprintln!("Eclipse: host toggle failed ({err})");
                }
                sync_host_menu(app, &host_for_menu);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(move |tray, event| {
            let app = tray.app_handle();
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => show_main_window(app),
                TrayIconEvent::Enter { .. } | TrayIconEvent::Click { .. } => {
                    sync_host_menu(app, &host_for_tray);
                }
                _ => {}
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if !argv.iter().any(|a| a == "--tray") {
                show_main_window(app);
            }
        }))
        .manage(HostProcess::new())
        .manage(SessionProcess::new())
        .invoke_handler(tauri::generate_handler![
            sidecar_status,
            list_peers,
            connect,
            disconnect,
            session_status,
            inbound_status,
            force_disconnect_inbound,
            peer_latency,
            set_stream_prefs,
            open_all_displays
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            // Packaged installs run Sunshine as a boot-time service. Attach if
            // :47990 is already up; spawn a child only for source/dev trees.
            app.state::<HostProcess>().start_sunshine();
            setup_tray(app)?;
            if !start_hidden() {
                show_main_window(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Eclipse")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<SessionProcess>().stop();
                // Kills only a child this process spawned; never a host service.
                app.state::<HostProcess>().stop_sunshine();
            }
        });
}
