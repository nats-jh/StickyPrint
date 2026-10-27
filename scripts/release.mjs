// 배포: 빌드(서명) → latest.json 만들기 → 공개 배포 저장소에 GitHub Release 로 올리기
//   npm run release -- "이번 버전에서 바뀐 점"
// 버전은 미리 tauri.conf.json, Cargo.toml, package.json 세 곳에서 올려 둔다.
// 버전을 올리지 않는 긴급 패치는 npm run hotfix -- "고친 점" (버전 뒤에 +1, +2 를 붙여 이 스크립트를 부른다)
import { copyFileSync, existsSync, readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { build } from './tauri-build.mjs';

const RELEASE_REPO = 'nats-jh/StickyPrint';
const root = new URL('../', import.meta.url);
const conf = JSON.parse(readFileSync(new URL('src-tauri/tauri.conf.json', root), 'utf8'));
const version = conf.version;
// 1.0.2+1 → 태그 v1.0.2-patch1, 파일 StickyPrint_1.0.2-patch1_x64-setup.exe (+ 는 주소에서 말썽이라 바꿔 올린다)
const [core, patchNo] = version.split('+');
const label = patchNo ? `${core}-patch${patchNo}` : core;
const tag = 'v' + label;
const notes = process.argv.slice(2).join(' ').trim() || 'StickyPrint ' + version;

const gh = (args, opts = {}) => spawnSync('gh', args, { encoding: 'utf8', shell: false, ...opts });

if (gh(['release', 'view', tag, '--repo', RELEASE_REPO]).status === 0) {
  console.error(`${tag} 는 이미 배포돼 있습니다. 버전을 올린 뒤 다시 실행하세요.`);
  process.exit(1);
}

build();

const dir = new URL('src-tauri/target/release/bundle/nsis/', root);
const builtName = `StickyPrint_${version}_x64-setup.exe`;
const setupName = `StickyPrint_${label}_x64-setup.exe`;
const sig = new URL(builtName + '.sig', dir);
const setup = new URL(setupName, dir);
if (builtName !== setupName) copyFileSync(new URL(builtName, dir), setup);
if (!existsSync(setup) || !existsSync(sig)) {
  console.error('설치 파일이나 서명(.sig)이 없습니다: ' + setup.pathname);
  process.exit(1);
}

// 앱은 항상 releases/latest/download/latest.json 을 보고, 거기 적힌 주소에서 설치 파일을 받는다
const latest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    'windows-x86_64': {
      signature: readFileSync(sig, 'utf8').trim(),
      url: `https://github.com/${RELEASE_REPO}/releases/download/${tag}/${setupName}`
    }
  }
};
const latestPath = new URL('latest.json', dir);
writeFileSync(latestPath, JSON.stringify(latest, null, 2));

const toPath = u => decodeURIComponent(u.pathname).replace(/^\/([A-Za-z]:)/, '$1');
const r = gh(['release', 'create', tag, toPath(setup), toPath(latestPath),
  '--repo', RELEASE_REPO, '--title', 'StickyPrint ' + core + (patchNo ? ' 패치 ' + patchNo : ''), '--notes', notes], { stdio: 'inherit' });
if (r.status !== 0) process.exit(r.status || 1);
console.log(`\n배포 완료: https://github.com/${RELEASE_REPO}/releases/tag/${tag}`);
