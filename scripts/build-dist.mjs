// web/ → dist/ : Tauri 가 앱 안에 넣을 화면 파일을 만든다.
// - 시작 파일은 index.html 이어야 한다
// - 템플릿은 <script> 로 읽으므로 .js 로 바꾼다 (앱 안 웹 엔진은 .txt 를 스크립트로 안 읽을 수 있음)
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';

const web = new URL('../web/', import.meta.url);
const dist = new URL('../dist/', import.meta.url);

rmSync(dist, { recursive: true, force: true });
mkdirSync(new URL('data/', dist), { recursive: true });

const html = readFileSync(new URL('StickyPrint.html', web), 'utf8');
const src = '<script src="data/템플릿.txt"></script>';
if (!html.includes(src)) throw new Error('StickyPrint.html 에서 템플릿 <script> 줄을 찾지 못했습니다');
writeFileSync(new URL('index.html', dist), html.replace(src, '<script src="data/templates.js"></script>'));

cpSync(new URL('data/', web), new URL('data/', dist), { recursive: true, filter: src => !src.endsWith('템플릿.txt') });
cpSync(new URL('data/템플릿.txt', web), new URL('data/templates.js', dist));
console.log('dist ready');
