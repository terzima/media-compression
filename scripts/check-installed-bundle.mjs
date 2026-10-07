import {spawnSync,spawn} from 'node:child_process';
import {readdir,readFile,mkdir,rm,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';

const windows=process.platform==='win32';
if(!windows&&process.platform!=='darwin')throw Error('Installation check requires macOS or Windows');
const target=process.env.MEDIA_BUILD_TARGET;
const bundle=path.resolve('target',...(target?[target]:[]),'release/bundle');
const destination=path.resolve('artifacts/install-check');
const mount=path.resolve('.tools/installer-mount');
const evidence={schemaVersion:1,platform:process.platform,target:target||process.arch,
  environment:'development/CI host; not clean minimum-OS qualification',checks:[]};
function run(program,args,env=process.env){const r=spawnSync(program,args,{env,stdio:'inherit'});if(r.status!==0)throw Error(`${path.basename(program)} failed (${r.status})`);}
async function find(root,name){for(const e of await readdir(root,{withFileTypes:true})){const p=path.join(root,e.name);if(e.isFile()&&e.name.toLowerCase()===name.toLowerCase())return p;if(e.isDirectory()){const found=await find(p,name);if(found)return found;}}return null;}
const hash=async p=>createHash('sha256').update(await readFile(p)).digest('hex');
await rm(destination,{recursive:true,force:true});await mkdir(destination,{recursive:true});
let application,codecs,worker;
if(windows){
  const files=await readdir(path.join(bundle,'nsis'));
  const installer=files.find(n=>n.endsWith('.exe'));if(!installer)throw Error('NSIS installer missing');
  const installerPath=path.join(bundle,'nsis',installer);
  // NSIS requires /D to be the final argument and has its own quoting rules.
  const command=`$p=Start-Process -FilePath $env:MEDIA_INSTALLER -ArgumentList @('/S',('/D='+$env:MEDIA_INSTALL_DEST)) -Wait -PassThru; exit $p.ExitCode`;
  evidence.installer={filename:installer,sha256:await hash(installerPath)};
  const template=await readFile(path.resolve('target',...(target?[target]:[]),'release/nsis/x64/installer.nsi'),'utf8');
  const runtime=template.match(/!define WEBVIEW2INSTALLERPATH "([^"\r\n]+)"/)?.[1]?.replaceAll('$$','$');
  if(!runtime)throw Error('Offline WebView2 installer was not embedded');
  const inspect=spawnSync('powershell.exe',['-NoProfile','-NonInteractive','-Command',
    '$v=(Get-Item $env:MEDIA_WEBVIEW_INSTALLER).VersionInfo; $s=Get-AuthenticodeSignature $env:MEDIA_WEBVIEW_INSTALLER; @{fileVersion=$v.FileVersion; productVersion=$v.ProductVersion; signatureStatus=[string]$s.Status; signatureMessage=$s.StatusMessage; signer=$s.SignerCertificate.Subject} | ConvertTo-Json -Compress'],
    {env:{...process.env,MEDIA_WEBVIEW_INSTALLER:runtime},encoding:'utf8'});
  if(inspect.status!==0)throw Error('WebView2 provenance inspection failed');
  evidence.webview2={filename:path.basename(runtime),sha256:await hash(runtime),...JSON.parse(inspect.stdout)};
  evidence.webview2.source='https://go.microsoft.com/fwlink/?linkid=2124701';
  evidence.webview2.qualifiedSignature=evidence.webview2.signatureStatus==='Valid'&&!!evidence.webview2.signer?.includes('Microsoft Corporation');
  await writeFile('artifacts/webview2-provenance.json',JSON.stringify(evidence.webview2,null,2)+'\n');
  console.log('Embedded WebView2 provenance:',JSON.stringify(evidence.webview2));
  if(evidence.webview2.signatureStatus==='HashMismatch'||(evidence.webview2.signatureStatus==='Valid'&&!evidence.webview2.qualifiedSignature))throw Error('Offline WebView2 installer integrity/publisher mismatch');
  if(!evidence.webview2.qualifiedSignature)console.log('WebView2 signature qualification remains a stable-publication gate; continuing development runtime checks.');
  run('powershell.exe',['-NoProfile','-NonInteractive','-Command',command],{...process.env,MEDIA_INSTALLER:installerPath,MEDIA_INSTALL_DEST:destination});
  application=await find(destination,'media-compression.exe')||await find(destination,'Media Compression.exe');
  const ffmpeg=await find(destination,'ffmpeg.exe');codecs=ffmpeg&&path.dirname(ffmpeg);
  worker=await find(destination,'media-worker.exe');
}else{
  const files=await readdir(path.join(bundle,'dmg'));
  const installer=files.find(n=>n.endsWith('.dmg'));if(!installer)throw Error('DMG missing');
  const installerPath=path.join(bundle,'dmg',installer);
  run('/usr/bin/hdiutil',['verify',installerPath]);await mkdir(mount,{recursive:true});
  run('/usr/bin/hdiutil',['attach',installerPath,'-readonly','-nobrowse','-mountpoint',mount]);
  try{run('/usr/bin/ditto',[path.join(mount,'Media Compression.app'),path.join(destination,'Media Compression.app')]);}
  finally{run('/usr/bin/hdiutil',['detach',mount]);}
  evidence.installer={filename:installer,sha256:await hash(installerPath)};
  const app=path.join(destination,'Media Compression.app/Contents');
  application=path.join(app,'MacOS/media-compression');
  codecs=path.join(app,'Resources/resources/codecs');worker=path.join(app,'MacOS/media-worker');
}
if(!application||!codecs||!worker)throw Error('Installed application/codec/worker missing');
evidence.checks.push('Installer copied/installed application and bundled helpers');
const ext=windows?'.exe':'';
for(const name of ['ffmpeg','ffprobe','pngquant','cwebp','cjpeg']){
  if(await hash(path.join(codecs,name+ext))!==await hash(path.resolve('src-tauri/resources/codecs',name+ext)))throw Error(`Installed ${name} differs from staged helper`);
}
evidence.checks.push('All five installed codec hashes match staged helpers');
run('cargo',['test','--locked','-p','media-engine','--test','integration','actual_','--','--include-ignored'],{...process.env,MEDIA_CODEC_DIR:codecs,MEDIA_WORKER_PATH:worker});
evidence.checks.push('Installed helpers passed image/audio studies, diagnostics, safe export and playback PCM preparation');
// CI launch smoke does not imply an interactive comparison or clean-machine test.
if(process.env.MEDIA_INSTALL_LAUNCH==='1'){
  const child=spawn(application,[],{stdio:'ignore'});
  let failed=null;child.once('error',e=>{failed=e;});
  await new Promise(resolve=>setTimeout(resolve,5000));
  if(failed||child.exitCode!==null)throw Error(`Installed desktop exited during launch: ${failed||child.exitCode}`);
  child.kill();
  evidence.checks.push('Installed desktop remained running during five-second launch smoke');
}
await writeFile('artifacts/installed-bundle-evidence.json',JSON.stringify(evidence,null,2)+'\n');
console.log('Installed-bundle checks passed; clean-machine GUI qualification remains separate.');
