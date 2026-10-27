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
  // 임시 파일에 다 쓴 뒤 바꿔 끼운다 — 쓰다가 꺼져도 기존 백업은 남는다
  fs::write(&tmp, data).map_err(|e| e.to_string())?;
  if path.exists() {
    let _ = fs::copy(&path, dir.join(BACKUP_PREV));
  }
  fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
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
    .invoke_handler(tauri::generate_handler![backup_read, backup_write])
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}
