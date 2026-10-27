use std::fs;
use std::path::PathBuf;
use tauri::Manager;

// 설정·템플릿 파일 백업.
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_updater::Builder::new().build())
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
    .invoke_handler(tauri::generate_handler![backup_read, backup_write, check_update, install_update])
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}
