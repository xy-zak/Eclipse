mod discovery;
mod display;
mod hostconf;
mod hostinfo;
mod pairing;
mod session;
mod sidecar;

use std::sync::Mutex;

use discovery::DiscoverySnapshot;
use pairing::InboundStream;
use session::{SessionProcess, SessionStatus, StreamPrefs};
use sidecar::{HostProcess, SidecarStatus};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State, WindowEvent};

static HOST_MENU: Mutex<Option<CheckMenuItem<tauri::Wry>>> = Mutex::new(None);

#[tauri::command]
async fn sidecar_status(app: tauri::AppHandle) -> SidecarStatus {
    tauri::async_runtime::spawn_blocking(move || {
        SidecarStatus::snapshot(&app.state::<HostProcess>())
    })
    .await
    .unwrap_or_else(|_| SidecarStatus {
        sunshine_present: false,
        moonlight_present: false,
        sunshine_running: false,
        keep_host_on_exit: false,
        host_triple: env!("HOST_TRIPLE").to_string(),
    })
}

#[tauri::command]
async fn list_peers() -> DiscoverySnapshot {
    tauri::async_runtime::spawn_blocking(discovery::list_peers)
        .await
        .unwrap_or_else(|err| {
            DiscoverySnapshot::fail(format!("Discovery task failed: {err}"))
        })
}

#[tauri::command]
fn session_status(session: State<SessionProcess>) -> SessionStatus {
    session.status()
}

#[tauri::command]
async fn inbound_status() -> Option<InboundStream> {
    tauri::async_runtime::spawn_blocking(pairing::local_inbound)
        .await
        .ok()
        .flatten()
}

#[tauri::command]
fn force_disconnect_inbound() -> Result<(), String> {
    pairing::force_disconnect_inbound()
}

#[tauri::command]
fn web_ui_password() -> Result<String, String> {
    crate::hostconf::read_web_ui_password()
}

#[tauri::command]
fn open_host_admin() -> Result<(), String> {
    tauri_plugin_opener::open_url("https://127.0.0.1:47990", None::<&str>)
        .map_err(|err| format!("Could not open host admin: {err}"))
}

#[tauri::command]
async fn set_host_running(app: tauri::AppHandle, on: bool) -> Result<(), String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || handle.state::<HostProcess>().set_host_running(on))
        .await
        .map_err(|err| format!("Host toggle failed: {err}"))??;
    if let Some(item) = HOST_MENU
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
    {
        sync_host_menu(&app, &item);
    }
    Ok(())
}

#[tauri::command]
fn set_keep_host_on_exit(app: tauri::AppHandle, on: bool) {
    app.state::<HostProcess>().set_keep_host_on_exit(on);
}

#[tauri::command]
fn disconnect(session: State<SessionProcess>) {
    session.stop();
}

#[tauri::command]
async fn peer_latency(ip: String) -> Option<u32> {
    tauri::async_runtime::spawn_blocking(move || discovery::peer_latency_ms(&ip))
        .await
        .ok()
        .flatten()
}

#[tauri::command]
async fn connect(
    app: tauri::AppHandle,
    ip: String,
    trust_new_cert: bool,
    displays: Option<Vec<crate::display::DisplayInfo>>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<SessionProcess>().connect(
            &ip,
            trust_new_cert,
            displays.unwrap_or_default(),
        )
    })
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
        "Eclipse — host streaming on · tailnet can connect".into()
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
    *HOST_MENU.lock().unwrap_or_else(|e| e.into_inner()) = Some(host_item.clone());
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
            web_ui_password,
            open_host_admin,
            set_host_running,
            set_keep_host_on_exit,
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
            // Off the UI thread: credential seeding and SCM queries can block.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                handle.state::<HostProcess>().start_sunshine();
            });
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
                let host = app.state::<HostProcess>();
                if !host.keep_host_on_exit() {
                    if let Err(err) = host.set_host_running(false) {
                        eprintln!("Eclipse: stop host on quit failed ({err})");
                        host.stop_sunshine();
                    }
                }
            }
        });
}
