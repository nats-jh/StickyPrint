# StickyPrint

A4 용지에 포스트잇을 붙여 프린터로 포스트잇에 바로 인쇄하는 Windows 프로그램.

## 설치

[Releases](https://github.com/nats-jh/StickyPrint/releases/latest)에서 `StickyPrint_x.y.z_x64-setup.exe`를 받아 실행한다.

- Windows 10, 11 (64비트), 관리자 권한 필요 없음
- 코드 서명을 하지 않아서 처음 실행 때 "Windows의 PC 보호"가 뜨면 **추가 정보 → 실행**
- 설치 후에는 켤 때마다 새 버전을 확인해 알려 준다. 인터넷이 없어도 사용에는 지장이 없다.

## 사용

1. **테두리 인쇄**: 빈 A4에 포스트잇 붙일 자리를 점선으로 찍는다
2. 점선에 맞춰 포스트잇을 붙인다
3. 같은 방향으로 다시 넣고 **내용 인쇄**

인쇄 버튼을 누르면 프린터만 고르면 된다. A4, 배율 100%, 여백 없음은 프로그램이 맞춘다. 창의 X를 누르면 바로 꺼지지 않고 한 번 묻는다.

테두리를 찍은 A4는 재활용하면 된다. 자세한 사용법은 프로그램 왼쪽 위 **사용법** 버튼.

## 폴더

| 경로 | 내용 |
|---|---|
| `web/StickyPrint.html` | 앱 화면 전체 (브라우저로 바로 열어도 동작) |
| `web/data/템플릿.txt` | 처음 들어 있는 기본 템플릿 |
| `web/data/*.png` | 화면에 쓰는 이미지 (로고 등, 아래 라이선스 참고) |
| `web/data/fonts/` | 앱에 넣은 한글 글꼴 6종 (OFL, 인터넷 없이 동작). 각 폴더에 라이선스 |
| `app-icon.png` | 아이콘 원본 1024px. `npx tauri icon app-icon.png` 로 `src-tauri/icons/` 생성 |
| `scripts/build-dist.mjs` | `web/` → `dist/` (index.html, 템플릿을 `templates.js` 로) |
| `scripts/tauri-build.mjs` | 서명 키를 넣고 `tauri build` |
| `scripts/release.mjs` | 빌드 → `latest.json` → GitHub Release 업로드 |
| `src-tauri/` | Tauri(Rust) 껍데기, 파일 백업, 자동 업데이트 |

## 빌드 (만드는 PC에만 필요)

필요: Node.js, Rust(rustup), Visual Studio C++ 빌드 도구, 업데이트 서명 키 `%USERPROFILE%\.tauri\stickyprint.key`
(저장소에 없음. 잃어버리면 기존 사용자에게 업데이트를 보낼 수 없으니 따로 백업).

```
npm install
npm run build
```

결과: `src-tauri/target/release/bundle/nsis/StickyPrint_x.y.z_x64-setup.exe` (+ `.sig`)

## 새 버전 배포

1. 버전 올리기. 세 곳을 같은 번호로: `src-tauri/tauri.conf.json` 의 `version`, `src-tauri/Cargo.toml`, `package.json`
2. `npm run release -- "이번에 바뀐 점"`
   빌드, 서명, `latest.json` 생성, 이 저장소의 GitHub Release 업로드까지 한 번에 한다 (`gh` 로그인 필요)
3. 설치된 프로그램은 다음에 켤 때 "새 버전이 있습니다" 안내를 띄우고, 누르면 받아서 설치한 뒤 다시 켜진다

### 긴급 패치 (버전을 올리지 않고)

`npm run hotfix -- "고친 점"`

버전 뒤에 패치 번호를 붙여(`1.0.2` → `1.0.2+1` → `1.0.2+2`) 바로 배포한다. 세 곳의 버전은 스크립트가 바꾼다.
- 앱 화면에는 `v1.0.2 (패치 1)` 로 보인다
- GitHub 에는 `v1.0.2-patch1` 태그, `StickyPrint_1.0.2-patch1_x64-setup.exe` 로 올라간다 (`+` 는 주소에서 말썽이라 바꿈)
- 앱은 `1.0.2 < 1.0.2+1 < 1.0.2+2 < 1.0.3` 순서로 새 버전을 알아본다 (`lib.rs` 의 `is_newer`)
- 다음 정식 버전은 세 곳을 `1.0.3` 처럼 `+` 없이 바꾸고 `npm run release`

앱은 `https://github.com/nats-jh/StickyPrint/releases/latest/download/latest.json` 을 본다.
다운로드 횟수: `gh release view vX.Y.Z --json assets -q '.assets[]|.name+" "+(.downloadCount|tostring)'`

### 설정과 템플릿이 유지되는 이유와 지켜야 할 것

- 앱 데이터는 프로그램 폴더가 아니라 사용자 폴더에 있다
  - 화면 저장소: `%LOCALAPPDATA%\com.kangjoonhyuk.stickyprint\EBWebView`
  - 파일 백업: `%APPDATA%\com.kangjoonhyuk.stickyprint\backup.json` (+ 직전 `backup.prev.json`)
  - 설정이 바뀔 때마다 1초 뒤 백업, 업데이트 직전에도 백업, 시작할 때 화면 저장소가 비어 있으면 자동 복구
- **바꾸면 안 되는 값**: `identifier`(`com.kangjoonhyuk.stickyprint`), `productName`, 업데이트 공개 키(`plugins.updater.pubkey`).
  바꾸면 다른 앱으로 인식되거나 업데이트가 거부된다.
- `web/StickyPrint.html` 의 `STORE_KEY`(`stickyprint-v1`)도 바꾸지 않는다. 저장 형식을 바꿔야 하면 키는 두고
  `loadState()` 에서 옛 형식을 새 형식으로 옮기는 코드를 추가한다.

## 개발 중 실행

```
npm run dev
```

## 라이선스

- **코드**: [MIT](LICENSE) © 2026 kangjoonhyuk
- **글꼴** (`web/data/fonts/`): 프리텐다드, 나눔고딕, 나눔손글씨 펜, 개구, 감자꽃, 주아. 모두 SIL Open Font License 1.1 (각 폴더의 LICENSE.txt)
- **이미지 (로고, 이름 이미지, 아이콘)**: MIT 대상이 아닙니다. © 2026 kangjoonhyuk, All rights reserved.
  `web/data/*.png`, `app-icon.png`, `src-tauri/icons/` 는 이 프로그램을 빌드하고 배포하는 용도로만 들어 있으며,
  다른 프로그램이나 목적에 쓰거나 고쳐 쓸 수 없습니다. 이 저장소를 바탕으로 다른 것을 만든다면 이미지는 직접 만들어 바꿔 주세요.
