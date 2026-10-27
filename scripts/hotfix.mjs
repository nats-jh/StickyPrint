// 긴급 패치: 버전은 그대로 두고 패치 번호만 올려서 바로 배포한다
//   npm run hotfix -- "고친 점"
// 1.0.2 → 1.0.2+1 → 1.0.2+2 ... 앱 화면에는 "v1.0.2 (패치 1)" 로 보인다.
// 다음 정식 버전(1.0.3)을 낼 때는 세 곳의 버전을 "1.0.3" 으로 바꾸면 패치 번호는 사라진다.
import { readFileSync, writeFileSync } from 'node:fs';

const root = new URL('../', import.meta.url);
const confUrl = new URL('src-tauri/tauri.conf.json', root);
const conf = JSON.parse(readFileSync(confUrl, 'utf8'));
const [core, patchNo] = conf.version.split('+');
const next = `${core}+${(parseInt(patchNo || '0', 10) || 0) + 1}`;

// 세 곳의 버전 줄만 바꾼다 (파일의 나머지 모양은 그대로)
const bump = (rel, pattern, line) => {
  const url = new URL(rel, root);
  const text = readFileSync(url, 'utf8');
  if (!pattern.test(text)) throw new Error(rel + ' 에서 버전 줄을 찾지 못했습니다');
  writeFileSync(url, text.replace(pattern, line));
};
bump('src-tauri/tauri.conf.json', /"version": "[^"]+"/, `"version": "${next}"`);
bump('package.json', /"version": "[^"]+"/, `"version": "${next}"`);
bump('src-tauri/Cargo.toml', /^version = "[^"]+"/m, `version = "${next}"`);
console.log(`긴급 패치 ${conf.version} → ${next}`);

await import('./release.mjs');
