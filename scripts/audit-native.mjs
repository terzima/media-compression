import { spawnSync } from 'node:child_process';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
const windows = process.platform === 'win32', ext = windows ? '.exe' : '';
const target = process.env.MEDIA_BUILD_TARGET;
const workerTarget = target || (windows ? 'x86_64-pc-windows-msvc' : process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin');
const root = path.resolve('target', ...(target ? [target] : []), 'release');
const files = ['ffmpeg','ffprobe','pngquant','cwebp','cjpeg'].map(n => path.resolve('src-tauri/resources/codecs', n + ext));
files.push(path.join(root,'media-compression'+ext), path.resolve('src-tauri/binaries',`media-worker-${workerTarget}${ext}`),path.resolve('src-tauri/binaries',`media-compression-agent-${workerTarget}${ext}`));
const evidence = [];
for (const file of files) {
  const result = spawnSync(windows ? 'objdump' : '/usr/bin/otool', windows ? ['-p',file] : ['-L',file], { encoding:'utf8',maxBuffer:64*1024*1024 });
  if (result.status !== 0) throw Error(`Dependency inspection failed: ${file}: ${result.error?.message||result.stderr}`);
  const dependencies = windows ? [...result.stdout.matchAll(/DLL Name:\s*(\S+)/g)].map(m => m[1]) : result.stdout.split('\n').slice(1).map(l => l.trim().split(' (')[0]).filter(Boolean);
  if (!dependencies.length) throw Error(`No native dependencies identified: ${file}`);
  const unsupported = dependencies.filter(d => windows ? /^(vcruntime|msvcp|libgcc|libstdc\+\+|libwinpthread)/i.test(d) : !d.startsWith('/usr/lib/') && !d.startsWith('/System/Library/'));
  if (unsupported.length) throw Error(`Unbundled native dependencies in ${file}: ${unsupported.join(', ')}`);
  evidence.push({ filename:path.basename(file), sha256:createHash('sha256').update(await readFile(file)).digest('hex'), dependencies, inspectionOutputBytes:Buffer.byteLength(result.stdout) });
}
await mkdir('artifacts',{recursive:true});
await writeFile('artifacts/native-dependencies.json',JSON.stringify({schemaVersion:1,platform:process.platform,target:target||process.arch,evidence},null,2)+'\n');
console.log('Native dependency inspection passed. Clean-machine runtime evidence remains separate.');
