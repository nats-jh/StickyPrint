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

// 파일 읽고 쓰기는 화면이 멈추지 않게 async 명령으로 (동기 명령은 메인 스레드에서 돈다)
#[tauri::command]
async fn backup_read(app: tauri::AppHandle) -> Result<Option<String>, String> {
  let dir = data_dir(&app)?;
  // 정전 등으로 비었거나 깨진 백업이면 직전 백업으로 넘어간다
  for name in [BACKUP, BACKUP_PREV] {
    if let Ok(text) = fs::read_to_string(dir.join(name)) {
      if serde_json::from_str::<serde_json::Value>(&text).is_ok() {
        return Ok(Some(text));
      }
    }
  }
  Ok(None)
}

#[tauri::command]
async fn backup_write(app: tauri::AppHandle, data: String) -> Result<(), String> {
  use std::io::Write;
  let dir = data_dir(&app)?;
  let path = dir.join(BACKUP);
  let tmp = dir.join("backup.json.tmp");
  // 임시 파일에 다 쓰고 디스크에 확실히 내린 뒤 바꿔 끼운다. 쓰다가 꺼져도 기존 백업은 남는다
  let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
  f.write_all(data.as_bytes()).and_then(|_| f.sync_all()).map_err(|e| e.to_string())?;
  drop(f);
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
// 설치 파일 받기는 느린 회선을 생각해 넉넉하게. 끊기면 "받는 중"에 멈춰 있지 않고 실패로 끝난다
const DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

// 버전을 올리지 않는 긴급 패치는 빌드 번호를 붙여 낸다: 1.0.2 < 1.0.2+1 < 1.0.2+2 < 1.0.3
// 기본 비교는 + 뒤를 무시하므로, 같은 버전이면 + 뒤 숫자로 한 번 더 비교한다
fn is_newer(current: semver::Version, remote: tauri_plugin_updater::RemoteRelease) -> bool {
  let patch_no = |v: &semver::Version| v.build.as_str().parse::<u64>().unwrap_or(0);
  match remote.version.cmp_precedence(&current) {
    std::cmp::Ordering::Greater => true,
    std::cmp::Ordering::Less => false,
    std::cmp::Ordering::Equal => patch_no(&remote.version) > patch_no(&current),
  }
}

#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
  use tauri_plugin_updater::UpdaterExt;
  let update = app
    .updater_builder()
    .timeout(UPDATE_TIMEOUT)
    .version_comparator(is_newer)
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
    .updater_builder()
    .timeout(DOWNLOAD_TIMEOUT)
    .version_comparator(is_newer)
    .build()
    .map_err(|e| e.to_string())?
    .check()
    .await
    .map_err(|e| e.to_string())?
    .ok_or("새 버전이 없습니다")?;
  // 받는 동안 화면에 진행률을 보여 주도록 알린다. 설치는 창 없이(quiet) 진행되고 끝나면 새 버전이 다시 켜진다
  use tauri::Emitter;
  let mut got: u64 = 0;
  let progress = app.clone();
  let done = app.clone();
  update
    .download_and_install(
      move |chunk, total| {
        got += chunk as u64;
        let _ = progress.emit("update-progress", (got, total));
      },
      move || {
        let _ = done.emit("update-installing", ());
      },
    )
    .await
    .map_err(|e| e.to_string())?;
  app.restart();
}

// 인쇄: 기본 인쇄 창을 띄우지 않고 고른 프린터로 바로 보낸다.
// A4, 배율 100%, 여백 0, 머리글/바닥글 없음을 고정해서 포스트잇 위치가 어긋나지 않게 한다.
#[derive(serde::Serialize)]
struct Printers {
  names: Vec<String>,
  default: Option<String>,
}

// 꺼진 네트워크 프린터를 찾느라 오래 걸릴 수 있어 별도 스레드에서
#[tauri::command]
async fn list_printers() -> Printers {
  tauri::async_runtime::spawn_blocking(|| {
    #[cfg(windows)]
    {
      let (names, default) = winprint::list();
      Printers { names, default }
    }
    #[cfg(not(windows))]
    Printers { names: vec![], default: None }
  })
  .await
  .unwrap_or(Printers { names: vec![], default: None })
}

// 고른 프린터가 컬러를 찍을 수 있는지. 모르면 None (꺼진 네트워크 프린터는 오래 걸릴 수 있어 목록과 따로 묻는다)
#[tauri::command]
async fn printer_color(printer: String) -> Option<bool> {
  tauri::async_runtime::spawn_blocking(move || {
    #[cfg(windows)]
    return winprint::is_color(&printer);
    #[cfg(not(windows))]
    {
      let _ = printer;
      None
    }
  })
  .await
  .unwrap_or(None)
}

#[tauri::command]
async fn print_page(window: tauri::WebviewWindow, printer: String, gray: bool) -> Result<(), String> {
  let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
  window
    .with_webview(move |webview| {
      #[cfg(windows)]
      winprint::print(webview, &printer, gray, tx);
      #[cfg(not(windows))]
      {
        let _ = (webview, printer, gray);
        let _ = tx.send(Err("이 운영체제에서는 바로 인쇄를 지원하지 않습니다".into()));
      }
    })
    .map_err(|e| e.to_string())?;
  // 프린터 응답을 기다리는 동안 다른 작업을 막지 않도록 별도 스레드에서 기다린다
  tauri::async_runtime::spawn_blocking(move || {
    rx.recv_timeout(std::time::Duration::from_secs(120))
      .unwrap_or_else(|_| Err("프린터가 응답하지 않습니다. 조금 늦게 인쇄될 수도 있으니 다시 인쇄하기 전에 프린터를 확인해 주세요".into()))
  })
  .await
  .map_err(|e| e.to_string())?
}

// 포스트잇을 화면에 실제 크기로 그리려고, 창이 있는 모니터의 실제 크기(mm)와 화면 점 수를 알려 준다.
// 모니터는 연결될 때 자기 크기를 EDID 로 알려 주고, Windows 는 그것을 레지스트리에 둔다.
// 모니터가 두 대면 지금 창이 있는 쪽을 본다. 크기를 못 읽으면 mm 는 비워 둔다 (화면에서 카드로 맞추게)
#[derive(serde::Serialize)]
struct MonitorInfo {
  id: String,
  width_px: u32,
  height_px: u32,
  width_mm: Option<u32>,
  height_mm: Option<u32>,
}

#[tauri::command]
async fn monitor_info(window: tauri::WebviewWindow) -> Result<MonitorInfo, String> {
  let monitor = window
    .current_monitor()
    .map_err(|e| e.to_string())?
    .ok_or("모니터를 찾지 못했습니다")?;
  let name = monitor.name().cloned().unwrap_or_default();
  let size = monitor.size();
  #[cfg(windows)]
  let (id, mm) = winmon::edid_size(&name);
  #[cfg(not(windows))]
  let (id, mm): (String, Option<(u32, u32)>) = (name.clone(), None);
  Ok(MonitorInfo {
    id: if id.is_empty() { name } else { id },
    width_px: size.width,
    height_px: size.height,
    width_mm: mm.map(|m| m.0),
    height_mm: mm.map(|m| m.1),
  })
}

#[cfg(windows)]
mod winmon {
  use windows::core::HSTRING;
  use windows::Win32::Graphics::Gdi::{EnumDisplayDevicesW, DISPLAY_DEVICEW};
  use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_BINARY};

  const EDD_GET_DEVICE_INTERFACE_NAME: u32 = 1;

  // "\\.\DISPLAY1" 같은 모니터 이름 → (모니터 구분 id, 실제 화면 크기 mm)
  pub fn edid_size(display_name: &str) -> (String, Option<(u32, u32)>) {
    let mut dd = DISPLAY_DEVICEW { cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32, ..Default::default() };
    let found = unsafe { EnumDisplayDevicesW(&HSTRING::from(display_name), 0, &mut dd, EDD_GET_DEVICE_INTERFACE_NAME) };
    if !found.as_bool() {
      return (String::new(), None);
    }
    // 예: \\?\DISPLAY#GSM5B09#5&2a0b1c2&0&UID4352#{e6f07b5f-...}
    let path = String::from_utf16_lossy(&dd.DeviceID[..dd.DeviceID.iter().position(|&c| c == 0).unwrap_or(dd.DeviceID.len())]);
    let parts: Vec<&str> = path.trim_start_matches(['\\', '?']).split('#').collect();
    if parts.len() < 3 {
      return (String::new(), None);
    }
    let id = format!("{}/{}", parts[1], parts[2]);
    let key = format!(r"SYSTEM\CurrentControlSet\Enum\{}\{}\{}\Device Parameters", parts[0], parts[1], parts[2]);
    let mut buf = vec![0u8; 1024];
    let mut len = buf.len() as u32;
    let err = unsafe {
      RegGetValueW(
        HKEY_LOCAL_MACHINE,
        &HSTRING::from(key),
        &HSTRING::from("EDID"),
        RRF_RT_REG_BINARY,
        None,
        Some(buf.as_mut_ptr() as *mut _),
        Some(&mut len),
      )
    };
    if err.is_err() {
      return (id, None);
    }
    buf.truncate(len as usize);
    (id, parse_edid_mm(&buf))
  }

  // EDID 에서 화면 크기(mm): 첫 번째 상세 타이밍(54번째 바이트부터)에 mm 단위로 있고, 없으면 21, 22번째의 cm 값
  pub fn parse_edid_mm(e: &[u8]) -> Option<(u32, u32)> {
    if e.len() < 128 || e[..8] != [0, 255, 255, 255, 255, 255, 255, 0] {
      return None;
    }
    let pixel_clock = e[54] as u32 | (e[55] as u32) << 8;
    if pixel_clock != 0 {
      let w = e[66] as u32 | ((e[68] as u32 & 0xF0) << 4);
      let h = e[67] as u32 | ((e[68] as u32 & 0x0F) << 8);
      if w > 0 && h > 0 {
        return Some((w, h));
      }
    }
    if e[21] > 0 && e[22] > 0 {
      return Some((e[21] as u32 * 10, e[22] as u32 * 10));
    }
    None
  }
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
      // 두 번 부르는 사이에 프린터가 늘면 버퍼가 모자라므로 한 번 더 시도한다
      for _ in 0..2 {
        if needed == 0 {
          break;
        }
        // PRINTER_INFO_4W 는 8바이트 정렬이 필요해 u64 로 잡는다
        let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
        let bytes = std::slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, needed as usize);
        if EnumPrintersW(flags, PCWSTR::null(), 4, Some(bytes), &mut needed, &mut count).is_ok() {
          let infos = std::slice::from_raw_parts(buf.as_ptr() as *const PRINTER_INFO_4W, count as usize);
          for info in infos {
            if let Ok(name) = info.pPrinterName.to_string() {
              names.push(name);
            }
          }
          break;
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

  pub fn is_color(printer: &str) -> Option<bool> {
    use windows::Win32::Storage::Xps::{DeviceCapabilitiesW, DC_COLORDEVICE};
    match unsafe { DeviceCapabilitiesW(&HSTRING::from(printer), PCWSTR::null(), DC_COLORDEVICE, None, None) } {
      1 => Some(true),
      0 => Some(false),
      _ => None,
    }
  }

  pub fn print(webview: tauri::webview::PlatformWebview, printer: &str, gray: bool, tx: Sender<Result<(), String>>) {
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
        settings2.SetColorMode(if gray { COREWEBVIEW2_PRINT_COLOR_MODE_GRAYSCALE } else { COREWEBVIEW2_PRINT_COLOR_MODE_COLOR })?;
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

#[cfg(test)]
mod tests {
  use super::is_newer;

  fn newer(current: &str, remote: &str) -> bool {
    let release: tauri_plugin_updater::RemoteRelease = serde_json::from_value(serde_json::json!({
      "version": remote, "url": "https://example.com/a.exe", "signature": "x"
    }))
    .unwrap();
    is_newer(semver::Version::parse(current).unwrap(), release)
  }

  #[cfg(windows)]
  #[test]
  fn edid_size_from_detailed_timing() {
    // 24인치급 모니터: 상세 타이밍에 527 x 296 mm
    let mut e = vec![0u8; 128];
    e[..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
    e[21] = 53;
    e[22] = 30;
    e[54] = 0x02;
    e[55] = 0x3a;
    e[66] = (527 & 0xFF) as u8;
    e[67] = (296 & 0xFF) as u8;
    e[68] = (((527 >> 8) << 4) | (296 >> 8)) as u8;
    assert_eq!(super::winmon::parse_edid_mm(&e), Some((527, 296)));
    e[54] = 0;
    e[55] = 0;
    assert_eq!(super::winmon::parse_edid_mm(&e), Some((530, 300)));
    assert_eq!(super::winmon::parse_edid_mm(&e[..100]), None);
  }

  // 이 PC 에 연결된 모니터에서 실제로 읽어 보기: cargo test --lib -- --ignored --nocapture
  #[cfg(windows)]
  #[test]
  #[ignore]
  fn print_connected_monitors() {
    for n in 1..=6 {
      let name = format!(r"\\.\DISPLAY{}", n);
      let (id, mm) = super::winmon::edid_size(&name);
      if !id.is_empty() {
        println!("{} {} {:?}", name, id, mm);
      }
    }
  }

  // 실제 PC 에서 확인용: cargo test print_printer_colors -- --ignored --nocapture
  #[cfg(windows)]
  #[test]
  #[ignore]
  fn print_printer_colors() {
    for name in super::winprint::list().0 {
      println!("{} {:?}", name, super::winprint::is_color(&name));
    }
  }

  #[test]
  fn hotfix_order() {
    assert!(newer("1.0.2", "1.0.2+1"));
    assert!(newer("1.0.2+1", "1.0.2+2"));
    assert!(newer("1.0.2+9", "1.0.2+10"));
    assert!(newer("1.0.2+3", "1.0.3"));
    assert!(newer("1.0.1", "1.0.2"));
    assert!(!newer("1.0.2", "1.0.2"));
    assert!(!newer("1.0.2+2", "1.0.2+1"));
    assert!(!newer("1.0.3", "1.0.2+5"));
    assert!(!newer("1.0.2+1", "1.0.2"));
  }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    // 이미 켜져 있으면 새로 띄우지 않고 기존 창을 앞으로 (여러 개가 같은 저장소를 덮어쓰지 않게). 가장 먼저 등록해야 한다
    .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
      if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
      }
    }))
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
      printer_color,
      print_page,
      monitor_info,
      quit_app
    ])
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}
