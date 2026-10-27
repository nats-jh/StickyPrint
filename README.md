# StickyPrint

A4 종이에 포스트잇을 붙여 프린터로 포스트잇에 바로 인쇄하는 도구. 화면은 html 한 장이고, Tauri 로 감싸 Windows 프로그램으로 배포한다.

## 폴더

| 경로 | 내용 |
|---|---|
| `web/StickyPrint.html` | 앱 화면 전체 (브라우저로 바로 열어도 동작) |
| `web/data/템플릿.txt` | 처음 들어 있는 기본 템플릿 |
| `web/data/logo.png` | 로고 (시작 화면·왼쪽 위) |
| `app-icon.png` | 아이콘 원본 1024px. `npx tauri icon app-icon.png` 로 `src-tauri/icons/` 생성 |
| `scripts/build-dist.mjs` | `web/` → `dist/` (index.html, 템플릿을 `templates.js` 로) |
| `src-tauri/` | Tauri(Rust) 껍데기와 설정 |

## 빌드 (만드는 PC에만 필요)

필요: Node.js, Rust(rustup), Visual Studio C++ 빌드 도구.

```
npm install
npm run build
```

결과:
- 설치 파일: `src-tauri/target/release/bundle/nsis/StickyPrint_1.0.0_x64-setup.exe`
- 설치 없이 실행: `src-tauri/target/release/stickyprint.exe`

받는 PC 에는 아무것도 필요 없다 (Windows 10·11 기본 WebView2 사용). 코드 서명을 안 했으므로 처음 실행 때
"Windows의 PC 보호" 가 뜨면 "추가 정보 → 실행".

## 업데이트 배포 (기존 설치 위에 덮어 설치)

1. 버전 올리기 — 세 곳을 같은 번호로: `src-tauri/tauri.conf.json` 의 `version`, `src-tauri/Cargo.toml`, `package.json`
2. `npm run build`
3. 새 `StickyPrint_x.y.z_x64-setup.exe` 를 전달 → 받는 사람은 **삭제하지 않고 그냥 실행**하면 기존 앱이 교체된다

설정·템플릿이 유지되는 이유와 지켜야 할 것:

- 앱 데이터는 프로그램 폴더가 아니라 사용자 폴더에 있다
  - 화면 저장소: `%LOCALAPPDATA%\com.kangjoonhyuk.stickyprint\EBWebView`
  - 파일 백업: `%APPDATA%\com.kangjoonhyuk.stickyprint\backup.json` (+ 직전 `backup.prev.json`)
  - 설정이 바뀔 때마다 1초 뒤 백업, 시작할 때 화면 저장소가 비어 있으면 백업에서 자동 복구
- **바꾸면 안 되는 값**: `identifier`(`com.kangjoonhyuk.stickyprint`), `productName`.
  바꾸면 다른 앱으로 인식돼 덮어 설치가 안 되고 저장소도 새로 시작한다.
- `web/StickyPrint.html` 의 `STORE_KEY`(`stickyprint-v1`)도 바꾸지 않는다. 저장 형식을 바꿔야 하면 키는 두고
  `loadState()` 에서 옛 형식을 새 형식으로 옮기는 코드를 추가한다.
- 제거 프로그램의 "앱 데이터 삭제" 를 체크하면 설정이 지워진다. 업데이트에는 제거가 필요 없다.

화면 왼쪽 아래에 `StickyPrint v1.0.0` 처럼 버전이 보인다 — 업데이트가 됐는지 여기서 확인.

## 개발 중 실행

```
npm run dev
```

© 2026 kangjoonhyuk
