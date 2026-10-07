import { readdir, readFile, writeFile, mkdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
const target = process.env.MEDIA_BUILD_TARGET;
const dir = path.resolve('target', ...(target ? [target] : []), 'release/bundle');
const artifacts = [];
async function walk(folder) {
  for (const item of await readdir(folder, { withFileTypes: true })) {
    const file = path.join(folder, item.name);
    if (item.isDirectory()) await walk(file);
    else if (/\.(dmg|exe)$/.test(item.name)) artifacts.push({
      filename: path.relative(dir, file).replaceAll('\\', '/'),
      bytes: (await readFile(file)).length,
      sha256: createHash('sha256').update(await readFile(file)).digest('hex'),
    });
  }
}
await walk(dir);
if (!artifacts.length) throw Error('No installer found; no checksums produced');
await mkdir('artifacts', { recursive: true });
await writeFile('artifacts/installer-checksums.sha256', artifacts.map(a => `${a.sha256}  ${a.filename}`).join('\n') + '\n');
await writeFile('artifacts/build-evidence.json', JSON.stringify({
  schemaVersion: 1, commit: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
  workingTreeChanges: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim() !== '',
  target: target || `${process.platform}-${process.arch}`, node: process.version,
  builtAt: new Date().toISOString(), qualification: 'unsigned development candidate', artifacts,
}, null, 2) + '\n');
console.log('Installer checksums and exact build evidence prepared.');
