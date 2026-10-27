// 서명 키를 넣고 tauri build 를 돌린다.
// 자동 업데이트용 설치 파일에는 서명(.sig)이 필요하다. 개인 키는 저장소가 아니라
// %USERPROFILE%\.tauri\stickyprint.key 에만 둔다 (잃어버리면 기존 사용자에게 업데이트를 보낼 수 없으니 백업).
import { existsSync, readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

export const KEY_PATH = join(homedir(), '.tauri', 'stickyprint.key');

export function build() {
  if (!existsSync(KEY_PATH)) {
    console.error('서명 키가 없습니다: ' + KEY_PATH + '\n백업해 둔 키를 이 위치에 넣어 주세요.');
    process.exit(1);
  }
  const env = {
    ...process.env,
    TAURI_SIGNING_PRIVATE_KEY: readFileSync(KEY_PATH, 'utf8').trim(),
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD: process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD || ''
  };
  const r = spawnSync('npx tauri build', { stdio: 'inherit', env, shell: true });
  if (r.status !== 0) process.exit(r.status || 1);
}

if (import.meta.url === `file:///${process.argv[1].replace(/\\/g, '/')}`) build();
