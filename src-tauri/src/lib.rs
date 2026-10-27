use std::fs;
use std::path::PathBuf;
use tauri::Manager;

// 설정, 템플릿 파일 백업.
// 화면의 저장소(WebView2 localStorage)가 업데이트나 정리로 비더라도 되살릴 수 있게
// %APPDATA%\com.kangjoonhyuk.stickyprint\backup.json 에 같은 내용을 둔다.
// 직전 백업은 backup.prev.json 으로 하나 더 남긴다.
const BACKUP: &str = "backup.json";
const BACKUP_PREV: &str = "backup.prev.json";

fn data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
  let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
  fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
  Ok(dir)
}

#[tauri::command]
fn backup_read(app: tauri::AppHandle) -> Result<Option<String>, String> {
  let dir = data_dir(&app)?;
  for name in [BACKUP, BACKUP_PREV] {
    if let Ok(text) = fs::read_to_string(dir.join(name)) {
      return Ok(Some(text));
    }
  }
  Ok(None)
}

#[tauri::command]
fn backup_write(app: tauri::AppHandle, data: String) -> Result<(), String> {
  let dir = data_dir(&app)?;
  let path = dir.join(BACKUP);
  let tmp = dir.join("backup.json.tmp");
  // 임시 파일에 다 쓴 뒤 바꿔 끼운다. 쓰다가 꺼져도 기존 백업은 남는다
  fs::write(&tmp, data).map_err(|e| e.to_string())?;
  if path.exists() {
    let _ = fs::copy(&path, dir.join(BACKUP_PREV));
  }
  fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

// 자동 업데이트.
// tauri.conf.json 의 endpoints(공개 배포 저장소의 latest.json)를 보고, 서명이 맞는 새 설치 파일만 받는다.
#[derive(serde::Serialize)]
struct UpdateInfo {
  version: String,
  notes: Option<String>,
}

// 인터넷이 없거나 느린 곳에서도 오래 붙잡히지 않게 10초 안에 끝낸다 (실패해도 앱 사용에는 영향 없음)
const UPDATE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
  use tauri_plugin_updater::UpdaterExt;
  let update = app
    .updater_builder()
    .timeout(UPDATE_TIMEOUT)
    .build()
    .map_err(|e| e.to_string())?
    .check()
    .await
    .map_err(|e| e.to_string())?;
  Ok(update.map(|u| UpdateInfo { version: u.version.clone(), notes: u.body.clone() }))
}

#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
  use tauri_plugin_updater::UpdaterExt;
  let update = app
    .updater()
    .map_err(|e| e.to_string())?
    .check()
    .await
    .map_err(|e| e.to_string())?
    .ok_or("새 버전이 없습니다")?;
  // Windows 는 설치 단계에서 앱이 닫히고, 설치가 끝나면 새 버전이 다시 켜진다
  update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
  app.restart();
}

// 인쇄: 기본 인쇄 창을 띄우지 않고 고른 프린터로 바로 보낸다.
// A4, 배율 100%, 여백 0, 머리글/바닥글 없음을 고정해서 포스트잇 위치가 어긋나지 않게 한다.
#[derive(serde::Serialize)]
struct Printers {
  names: Vec<String>,
  default: Option<String>,
}

#[tauri::command]
fn list_printers() -> Printers {
  #[cfg(windows)]
  {
    let (names, default) = winprint::list();
    Printers { names, default }
  }
  #[cfg(not(windows))]
  Printers { names: vec![], default: None }
}

#[tauri::command]
async fn print_page(window: tauri::WebviewWindow, printer: String) -> Result<(), String> {
  let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
  window
    .with_webview(move |webview| {
      #[cfg(windows)]
      winprint::print(webview, &printer, tx);
      #[cfg(not(windows))]
      {
        let _ = (webview, printer);
        let _ = tx.send(Err("이 운영체제에서는 바로 인쇄를 지원하지 않습니다".into()));
      }
    })
    .map_err(|e| e.to_string())?;
  // 프린터 응답을 기다리는 동안 다른 작업을 막지 않도록 별도 스레드에서 기다린다
  tauri::async_runtime::spawn_blocking(move || {
    rx.recv_timeout(std::time::Duration::from_secs(120))
      .unwrap_or_else(|_| Err("프린터가 응답하지 않습니다".into()))
  })
  .await
  .map_err(|e| e.to_string())?
}

#[cfg(windows)]
mod winprint {
  use std::sync::mpsc::Sender;
  use webview2_com::Microsoft::Web::WebView2::Win32::*;
  use webview2_com::PrintCompletedHandler;
  use windows::core::{Interface, HSTRING, PCWSTR, PWSTR};
  use windows::Win32::Graphics::Printing::*;

  const A4_WIDTH_IN: f64 = 210.0 / 25.4;
  const A4_HEIGHT_IN: f64 = 297.0 / 25.4;

  pub fn list() -> (Vec<String>, Option<String>) {
    let mut names = Vec::new();
    unsafe {
      let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
      let (mut needed, mut count) = (0u32, 0u32);
      let _ = EnumPrintersW(flags, PCWSTR::null(), 4, None, &mut needed, &mut count);
      if needed > 0 {
        let mut buf = vec![0u8; needed as usize];
        if EnumPrintersW(flags, PCWSTR::null(), 4, Some(&mut buf), &mut needed, &mut count).is_ok() {
          let infos = std::slice::from_raw_parts(buf.as_ptr() as *const PRINTER_INFO_4W, count as usize);
          for info in infos {
            if let Ok(name) = info.pPrinterName.to_string() {
              names.push(name);
            }
          }
        }
      }
      let mut len = 0u32;
      let _ = GetDefaultPrinterW(None, &mut len);
      let default = if len > 0 {
        let mut b = vec![0u16; len as usize];
        if GetDefaultPrinterW(Some(PWSTR(b.as_mut_ptr())), &mut len).as_bool() {
          Some(String::from_utf16_lossy(&b[..(len as usize).saturating_sub(1)]))
        } else {
          None
        }
      } else {
        None
      };
      (names, default)
    }
  }

  pub fn print(webview: tauri::webview::PlatformWebview, printer: &str, tx: Sender<Result<(), String>>) {
    let fail = tx.clone();
    let result = (|| -> windows::core::Result<()> {
      unsafe {
        let core: ICoreWebView2_16 = webview.controller().CoreWebView2()?.cast()?;
        let env: ICoreWebView2Environment6 = webview.environment().cast()?;
        let settings = env.CreatePrintSettings()?;
        settings.SetOrientation(COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT)?;
        settings.SetPageWidth(A4_WIDTH_IN)?;
        settings.SetPageHeight(A4_HEIGHT_IN)?;
        settings.SetMarginTop(0.0)?;
        settings.SetMarginBottom(0.0)?;
        settings.SetMarginLeft(0.0)?;
        settings.SetMarginRight(0.0)?;
        settings.SetScaleFactor(1.0)?;
        settings.SetShouldPrintHeaderAndFooter(false)?;
        settings.SetShouldPrintBackgrounds(true)?;
        let settings2: ICoreWebView2PrintSettings2 = settings.cast()?;
        settings2.SetPrinterName(&HSTRING::from(printer))?;
        settings2.SetCopies(1)?;
        let handler = PrintCompletedHandler::create(Box::new(move |error, status| {
          let result = match error {
            Err(e) => Err(e.message().to_string()),
            Ok(()) if status == COREWEBVIEW2_PRINT_STATUS_SUCCEEDED => Ok(()),
            Ok(()) if status == COREWEBVIEW2_PRINT_STATUS_PRINTER_UNAVAILABLE => Err("프린터를 찾을 수 없습니다. 프린터가 켜져 있는지 확인해 주세요".into()),
            Ok(()) => Err("인쇄하지 못했습니다".into()),
          };
          let _ = tx.send(result);
          Ok(())
        }));
        core.Print(&settings, &handler)?;
      }
      Ok(())
    })();
    if let Err(e) = result {
      let _ = fail.send(Err(e.message().to_string()));
    }
  }
}

// 창의 X: 바로 닫지 않고 화면에 "끌까요?"를 묻는다. 확인 창이 떠 있는 동안 X를 한 번 더 누르면 바로 닫힌다.
static QUITTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static LAST_CLOSE_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
const CLOSE_AGAIN_MS: u64 = 3000;

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
  QUITTING.store(true, std::sync::atomic::Ordering::SeqCst);
  app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_updater::Builder::new().build())
    .on_window_event(|window, event| {
      use std::sync::atomic::Ordering;
      use tauri::Emitter;
      if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        if QUITTING.load(Ordering::SeqCst) {
          return;
        }
        let now = std::time::SystemTime::now()
          .duration_since(std::time::UNIX_EPOCH)
          .map(|d| d.as_millis() as u64)
          .unwrap_or(0);
        let last = LAST_CLOSE_MS.swap(now, Ordering::SeqCst);
        if now.saturating_sub(last) < CLOSE_AGAIN_MS {
          return;
        }
        api.prevent_close();
        let _ = window.emit("close-requested", ());
      }
    })
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      backup_read,
      backup_write,
      check_update,
      install_update,
      list_printers,
      print_page,
      quit_app
    ])
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}
