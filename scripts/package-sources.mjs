import {spawnSync} from 'node:child_process';import{mkdir,cp,readFile,writeFile,readdir,rm}from'node:fs/promises';import{createHash}from'node:crypto';import{homedir}from'node:os';import path from'node:path';
const out=path.resolve('artifacts');await mkdir(out,{recursive:true});const material=path.join(out,'dependency-source');await rm(material,{recursive:true,force:true});await mkdir(material,{recursive:true});
const cargo=path.join(homedir(),'.cargo/bin/cargo'+(process.platform==='win32'?'.exe':''));
function run(cmd,args){const p=spawnSync(process.platform==='win32'&&cmd==='tar'?path.join(process.env.SystemRoot||'C:/Windows','System32/tar.exe'):cmd,args,{encoding:'utf8',maxBuffer:64*1024*1024});if(p.status!==0)throw Error(p.stderr||p.error);return p.stdout;}
const patchArgs=['--config',`patch.crates-io.lcms2-sys.path="${path.resolve('.tools/patched-sys/lcms2-sys').replaceAll('\\','/')}"`,'--config',`patch.crates-io.libpng-sys.path="${path.resolve('.tools/patched-sys/libpng-sys').replaceAll('\\','/')}"`];
const metadata=JSON.parse(run(cargo,['metadata','--locked','--format-version','1']));
const pngMetadata=JSON.parse(run(cargo,['metadata','--locked','--format-version','1','--manifest-path','.tools/native/pngquant-3.0.3/Cargo.toml',...patchArgs]));
const packages=new Map([...metadata.packages,...pngMetadata.packages].filter(p=>p.source||p.name.endsWith('-sys')).map(p=>[p.name+'-'+p.version,p]));
const supplements=JSON.parse(await readFile('scripts/license-supplements.json','utf8'));
const licenseReview=[];
const dependencies=[];const notices=['MEDIA COMPRESSION — THIRD-PARTY NOTICES','Project-authored code: MIT (see LICENSE). Dependencies retain their own licenses.','Native tools are separate executables; pngquant/libimagequant are GPL-3.0-or-later.','FFmpeg is built without GPL/nonfree features. See scripts/build-codecs.mjs and the manifest.',''];
for(const p of packages.values()){
 const dir=path.dirname(p.manifest_path);if(!p.license)throw Error(`Missing license declaration: ${p.name}`);
 dependencies.push({name:p.name,version:p.version,source:p.source,license:p.license,checksum:await readFile(path.join(dir,'.cargo-checksum.json')).then(b=>JSON.parse(b).package).catch(()=>null)});
 notices.push(`${p.name} ${p.version}: ${p.license}${p.authors?.length?' — '+p.authors.join(', '):''}`);
 const names=(await readdir(dir)).filter(n=>/^(LICEN[CS]E|COPYING|NOTICE|COPYRIGHT)/i.test(n));
 if(!names.length&&['lcms2-sys','libpng-sys'].includes(p.name))names.push('vendor/LICENSE');
 const item=path.join(material,'licenses','rust',p.name+'-'+p.version);await mkdir(item,{recursive:true});
 for(const name of names){await mkdir(path.dirname(path.join(item,name)),{recursive:true});await cp(path.join(dir,name),path.join(item,name),{recursive:true});}
 const supplement=supplements.find(s=>s.name===p.name&&s.version===p.version);
 if(!names.length){
  if(!supplement)throw Error(`License text missing; review upstream before distribution: ${p.name} ${p.version}`);
  const data=await readFile(path.join('scripts',supplement.file));
  if(createHash('sha256').update(data).digest('hex')!==supplement.sha256)throw Error(`License supplement changed: ${p.name}`);
  await writeFile(path.join(item,'UPSTREAM_LICENSE.txt'),data);
  if(supplement.standardFile){const standard=await readFile(path.join('scripts',supplement.standardFile));if(createHash('sha256').update(standard).digest('hex')!==supplement.standardSha256)throw Error('Standard license text changed');await writeFile(path.join(item,'STANDARD_LICENSE.txt'),standard);}
  notices.push(`  License text from ${supplement.url}`);
 }
 licenseReview.push({name:p.name,version:p.version,license:p.license,texts:names.length?names:['UPSTREAM_LICENSE.txt'],...(names.length?{}:{upstream:supplement.url,sha256:supplement.sha256})});
}
run(cargo,['vendor','--locked',path.join(material,'rust-vendor')]);
await writeFile(path.join(material,'rust-vendor-config.toml'),'[source.crates-io]\nreplace-with = "vendored-sources"\n[source.vendored-sources]\ndirectory = "rust-vendor"\n');
const sources=JSON.parse(await readFile('scripts/codec-sources.json'));for(const s of sources){const archive=await readFile('.tools/sources/'+s.name);if(createHash('sha256').update(archive).digest('hex')!==s.sha256)throw Error('Source archive changed');await cp('.tools/sources/'+s.name,path.join(material,s.name));notices.push(`${s.name}: ${s.license} — ${s.url}`);}
await cp('src-tauri/resources/codecs/licenses',path.join(material,'licenses','native'),{recursive:true,filter:src=>path.basename(src)!=='dependencies'});
await cp('src-tauri/resources/codecs/manifest.json',path.join(material,'codec-build-manifest.json'));
const codecManifest=JSON.parse(await readFile('src-tauri/resources/codecs/manifest.json','utf8'));
const flags=codecManifest.build.ffmpegFlags;
if(!flags.includes('--disable-gpl')||!flags.includes('--disable-nonfree')||flags.some(f=>f==='--enable-gpl'||f==='--enable-nonfree'))throw Error('FFmpeg configuration is outside the reviewed LGPL build');
for(const s of sources){
 const prefix=s.name.replace(/\.tar\.(gz|xz)$/,'')+'-';
 if(!(await readdir(path.join(material,'licenses','native'))).some(n=>n.startsWith(prefix)))throw Error(`Native license missing: ${s.name}`);
}
// pngquant's patched bindings and locked registry dependencies belong in matching source.
const png=path.join(material,'pngquant-vendor');run(cargo,['vendor','--locked','--manifest-path','.tools/native/pngquant-3.0.3/Cargo.toml','--config',`patch.crates-io.lcms2-sys.path="${path.resolve('.tools/patched-sys/lcms2-sys').replaceAll('\\','/')}"`,'--config',`patch.crates-io.libpng-sys.path="${path.resolve('.tools/patched-sys/libpng-sys').replaceAll('\\','/')}"`,png]);await writeFile(path.join(material,'pngquant-vendor-config.toml'),'[source.crates-io]\nreplace-with = "vendored-sources"\n[source.vendored-sources]\ndirectory = "pngquant-vendor"\n');await cp('.tools/patched-sys',path.join(material,'patched-sys'),{recursive:true,filter:src=>!['build','target'].includes(path.basename(src))});
const npm=JSON.parse(await readFile('package-lock.json'));for(const [name,p]of Object.entries(npm.packages)){if(!name||p.dev)continue;dependencies.push({name,version:p.version,license:p.license,resolved:p.resolved,integrity:p.integrity});notices.push(`${name} ${p.version}: ${p.license??'SEE SOURCE'}`);for(const file of await readdir(name)){if(/^(LICEN[CS]E|NOTICE|COPYRIGHT)/i.test(file)){const dest=path.join(material,'licenses','npm',name);await mkdir(dest,{recursive:true});await cp(path.join(name,file),path.join(dest,file),{recursive:true});}}}
for(const name of ['scripts','.cargo','Cargo.lock','Cargo.toml','package-lock.json','rust-toolchain.toml','LICENSE','docs/PROVENANCE.md','docs/SOURCE_BUILD.md'])await cp(name,path.join(material,name),{recursive:true});
const review={schemaVersion:1,scope:'Source/license-text packaging checks; not patent clearance or stable-release qualification',rust:licenseReview,native:sources.map(s=>({name:s.name,sha256:s.sha256,license:s.license})),ffmpegFlags:flags};
await writeFile(path.join(out,'distribution-review.json'),JSON.stringify(review,null,2)+'\n');
await writeFile(path.join(material,'distribution-review.json'),JSON.stringify(review,null,2)+'\n');
await writeFile(path.join(out,'dependency-manifest.json'),JSON.stringify({schemaVersion:1,dependencies,sources},null,2)+'\n');await writeFile(path.join(out,'THIRD_PARTY_NOTICES.txt'),notices.join('\n')+'\n');await cp(path.join(out,'THIRD_PARTY_NOTICES.txt'),'src-tauri/resources/codecs/THIRD_PARTY_NOTICES.txt');await cp(path.join(material,'licenses'),'src-tauri/resources/codecs/licenses/dependencies',{recursive:true});
const archive=path.join(out,'dependency-source.tar.gz');run('tar',['-czf',archive,'-C',out,'dependency-source']);await writeFile(path.join(out,'dependency-source.sha256'),createHash('sha256').update(await readFile(archive)).digest('hex')+'  dependency-source.tar.gz\n');
console.log('Matching source, dependency manifest, notices, and checksums prepared. No release has been published.');
