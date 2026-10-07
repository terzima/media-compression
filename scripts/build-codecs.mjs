import { readFile, writeFile, mkdir, cp, chmod, readdir, rm } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import os from 'node:os';

const root=path.resolve(import.meta.dirname,'..');
const sourceDir=path.join(root,'.tools/sources'), buildDir=path.join(root,'.tools/native');
const prefix=path.join(buildDir,'prefix'), dest=path.join(root,'src-tauri/resources/codecs');
const windows=process.platform==='win32', ext=windows?'.exe':'';
const env={...process.env,...(windows?{CC:'gcc',CXX:'g++'}:{}),PATH:[path.join(prefix,'bin'),path.join(root,'.tools/build-env',windows?'Scripts':'bin'),path.join(os.homedir(),'.cargo/bin'),process.env.PATH].join(path.delimiter),PKG_CONFIG_PATH:path.join(prefix,'lib/pkgconfig'),MEDIA_CODEC_PREFIX:prefix,CFLAGS:windows?'-O2':'-O2 -mmacosx-version-min=14.0',CXXFLAGS:windows?'-O2':'-O2 -mmacosx-version-min=14.0',LDFLAGS:windows?'-static -static-libgcc':'-mmacosx-version-min=14.0',MACOSX_DEPLOYMENT_TARGET:'14.0'};
function run(command,args,cwd=root) {
  console.log(`${command} ${args.join(' ')}`);
  const executable=windows&&command==='tar'?path.join(process.env.SystemRoot||'C:/Windows','System32/tar.exe'):command;
  const result=spawnSync(executable,windows?args.map(a=>a.replaceAll('\\','/')):args,{cwd,env,stdio:'inherit'});
  if(result.error)throw result.error;
  if(result.status!==0)throw new Error(`${command} failed (${result.status})`);
}
await mkdir(sourceDir,{recursive:true}); await mkdir(prefix,{recursive:true});await mkdir(dest,{recursive:true});
await mkdir(path.join(prefix,'bin'),{recursive:true});
// C build tools are selected explicitly on Windows; Rust's cc crate uses MSVC.

const pkgWrapper=path.join(prefix,'bin/pkg-config');
await writeFile(pkgWrapper,`#!/bin/sh\nexec node '${path.join(root,'scripts/pkg-config.mjs').replaceAll("'","'\\''")}' "$@"\n`);await chmod(pkgWrapper,0o755);
const sources=JSON.parse(await readFile(path.join(root,'scripts/codec-sources.json'),'utf8'));
for(const source of sources){
 const archive=path.join(sourceDir,source.name);
 if(!existsSync(archive)){
  const response=await fetch(source.url);if(!response.ok)throw new Error(`Download failed: ${source.name} (${response.status})`);
  await writeFile(archive,Buffer.from(await response.arrayBuffer()));
 }
 const sha=createHash('sha256').update(await readFile(archive)).digest('hex');
 if(sha!==source.sha256)throw new Error(`Source checksum mismatch: ${source.name}`);
 const dir=path.join(buildDir,source.name.replace(/\.tar\.(gz|xz)$/,''));
 if(!existsSync(dir)){await mkdir(buildDir,{recursive:true});run('tar',['-xf',archive,'-C',buildDir]);}
}
function cmake(name, options=[]){
 const src=path.join(buildDir,name),out=path.join(src,'build');
 run('cmake',['-S',src,'-B',out,'-G','Ninja','-DCMAKE_BUILD_TYPE=Release',`-DCMAKE_INSTALL_PREFIX=${prefix}`,'-DCMAKE_INSTALL_LIBDIR=lib','-DCMAKE_OSX_DEPLOYMENT_TARGET=14.0','-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded',...options]);
 run('cmake',['--build',out,'--parallel','4']);run('cmake',['--install',out]);
}
if(!existsSync(path.join(prefix,'lib/libopus.a')))cmake('opus-1.6.1',['-DBUILD_SHARED_LIBS=OFF','-DOPUS_BUILD_PROGRAMS=OFF','-DOPUS_BUILD_TESTING=OFF']);
if(!existsSync(path.join(prefix,'lib/libmp3lame.a'))){
 const src=path.join(buildDir,'lame-4.0');
 run('sh',['configure',`--prefix=${prefix}`,'--disable-shared','--enable-static','--disable-decoder','--disable-frontend'],src);
 run('make',['-j4'],src);run('make',['install'],src);
}
if(!existsSync(path.join(prefix,'lib/libz.a')))cmake('zlib-1.3.2',['-DZLIB_BUILD_SHARED=OFF','-DZLIB_BUILD_TESTING=OFF']);
if(windows && existsSync(path.join(prefix,'lib/libzs.a')))await cp(path.join(prefix,'lib/libzs.a'),path.join(prefix,'lib/libz.a'));
if(!existsSync(path.join(prefix,'lib/libpng16.a')))cmake('libpng-1.6.59',['-DPNG_SHARED=OFF','-DPNG_TESTS=OFF',`-DZLIB_LIBRARY=${prefix}/lib/libz.a`,`-DZLIB_INCLUDE_DIR=${prefix}/include`]);
if(!existsSync(path.join(prefix,'bin/cwebp'+ext)))cmake('libwebp-1.6.0',['-DBUILD_SHARED_LIBS=OFF','-DWEBP_BUILD_CWEBP=ON','-DWEBP_BUILD_DWEBP=OFF','-DWEBP_BUILD_EXTRAS=OFF','-DWEBP_BUILD_ANIM_UTILS=OFF','-DWEBP_BUILD_VWEBP=OFF','-DWEBP_BUILD_WEBPMUX=OFF','-DWEBP_BUILD_GIF2WEBP=OFF',`-DPNG_LIBRARY=${prefix}/lib/libpng16.a`,`-DPNG_PNG_INCLUDE_DIR=${prefix}/include`,`-DZLIB_LIBRARY=${prefix}/lib/libz.a`,`-DZLIB_INCLUDE_DIR=${prefix}/include`,'-DWEBP_ENABLE_SIMD=ON']);
if(!existsSync(path.join(prefix,'bin/cjpeg'+ext)))cmake('libjpeg-turbo-3.2.0',['-DENABLE_SHARED=OFF','-DENABLE_STATIC=ON','-DWITH_TURBOJPEG=OFF','-DWITH_SIMD=OFF']);
const imagequant=sources.find(s=>s.name.startsWith('libimagequant-'));
await cp(path.join(buildDir,imagequant.name.replace(/\.tar\.(gz|xz)$/,'')),path.join(buildDir,'pngquant-3.0.3/lib'),{recursive:true});
await cp(path.join(root,'scripts/pngquant.Cargo.lock'),path.join(buildDir,'pngquant-3.0.3/Cargo.lock'));
run('cargo',['fetch','--manifest-path',path.join(buildDir,'pngquant-3.0.3/Cargo.toml')]);
const patches=path.join(root,'.tools/patched-sys');await mkdir(patches,{recursive:true});
run('cargo',['fetch','--locked']);
const registry=path.join(os.homedir(),'.cargo/registry/src');
for(const [name,version,upstream] of [['lcms2-sys','4.0.7','Little-CMS-lcms2.19.1'],['libpng-sys','1.1.11','libpng-1.6.59']]){
 const target=path.join(patches,name);
 if(!existsSync(target)){
  let original;for(const folder of await readdir(registry)){const candidate=path.join(registry,folder,`${name}-${version}`);if(existsSync(candidate))original=candidate;}
  if(!original)throw new Error(`Missing locked ${name} binding; fetch pngquant dependencies first`);
  await cp(original,target,{recursive:true});await rm(path.join(target,'vendor'),{recursive:true,force:true});await cp(path.join(buildDir,upstream),path.join(target,'vendor'),{recursive:true});
 }
}
await cp(path.join(root,'scripts/pngquant.Cargo.lock'),path.join(buildDir,'pngquant-3.0.3/Cargo.lock'));
const patchArgs=['--config',`patch.crates-io.lcms2-sys.path="${path.join(patches,'lcms2-sys').replaceAll('\\','/')}"`,'--config',`patch.crates-io.libpng-sys.path="${path.join(patches,'libpng-sys').replaceAll('\\','/')}"`];
if(!existsSync(path.join(buildDir,'pngquant-3.0.3/target/release/pngquant'+ext))){
 const imagequant=sources.find(s=>s.name.startsWith('libimagequant-'));
 if(!imagequant)throw new Error('Pinned libimagequant source missing');
 await cp(path.join(buildDir,imagequant.name.replace(/\.tar\.(gz|xz)$/,'')),path.join(buildDir,'pngquant-3.0.3/lib'),{recursive:true});
 await cp(path.join(root,'scripts/pngquant.Cargo.lock'),path.join(buildDir,'pngquant-3.0.3/Cargo.lock'));
 const rustEnvCC=env.CC,rustEnvCXX=env.CXX;if(windows){delete env.CC;delete env.CXX;}
 run('cargo',['build','--release','--locked','--features','static,z-static',...patchArgs,'--manifest-path',path.join(buildDir,'pngquant-3.0.3/Cargo.toml')]);
 if(rustEnvCC)env.CC=rustEnvCC;if(rustEnvCXX)env.CXX=rustEnvCXX;
}
if(!existsSync(path.join(prefix,'bin/ffmpeg'+ext))){
 const src=path.join(buildDir,'ffmpeg-8.1.3');
 const pkg=path.join(buildDir,'pkg-config');
 await writeFile(pkg,`#!/bin/sh\nexec node '${path.join(root,'scripts/pkg-config.mjs').replaceAll("'","'\\''")}' "$@"\n`);await chmod(pkg,0o755);
 run('sh',['configure',`--prefix=${prefix}`,`--pkg-config=${pkg}`,...(windows?['--target-os=mingw32','--arch=x86_64','--cc=gcc','--cxx=g++']:[]),'--disable-gpl','--disable-nonfree','--disable-network','--disable-shared','--enable-static','--disable-doc','--disable-debug','--disable-autodetect','--disable-avdevice','--disable-sdl2','--disable-x86asm','--enable-libopus','--enable-libmp3lame',`--extra-cflags=-I${prefix}/include`,`--extra-ldflags=-L${prefix}/lib${windows?' -static -static-libgcc':''}`],src);
 run('make',['-j4'],src);run('make',['install'],src);
}
for(const name of ['ffmpeg','ffprobe','cwebp','cjpeg'])await cp(path.join(prefix,'bin',name+ext),path.join(dest,name+ext));
await cp(path.join(buildDir,'pngquant-3.0.3/target/release/pngquant'+ext),path.join(dest,'pngquant'+ext));
const versions={platform:process.platform,arch:process.arch,sources,helpers:{},build:{node:process.version,recipeSha256:createHash('sha256').update(await readFile(path.join(root,'scripts/build-codecs.mjs'))).digest('hex'),compiler:spawnSync(env.CC||'cc',['--version'],{encoding:'utf8',env}).stdout?.split('\n')[0],cflags:env.CFLAGS,ldflags:env.LDFLAGS,ffmpegFlags:'--disable-gpl --disable-nonfree --disable-network --disable-autodetect --enable-libopus --enable-libmp3lame'}};
for(const name of ['ffmpeg','ffprobe','pngquant','cwebp','cjpeg']){
 const binary=path.join(dest,name+ext);await chmod(binary,0o755);
 const args=name==='ffmpeg'||name==='ffprobe'?['-version']:name==='pngquant'?['--version']:['-version'];
 const output=spawnSync(binary,args,{encoding:'utf8'});
 if(output.status!==0)throw new Error(`Bundled ${name} does not launch`);
 versions.helpers[name]={sha256:createHash('sha256').update(await readFile(binary)).digest('hex'),version:(output.stdout||output.stderr).split('\n')[0]};
}
await writeFile(path.join(dest,'manifest.json'),JSON.stringify(versions,null,2)+'\n');
const notices=path.join(dest,'licenses');await mkdir(notices,{recursive:true});
for(const source of sources){
 const dir=path.join(buildDir,source.name.replace(/\.tar\.(gz|xz)$/,''));
 for(const file of await readdir(dir))if(/^(COPYING|COPYRIGHT|LICENSE|PATENTS|README\.ijg)/.test(file))await cp(path.join(dir,file),path.join(notices,path.basename(dir)+'-'+file),{recursive:true});
}
await cp(path.join(root,'LICENSE'),path.join(dest,'PROJECT_LICENSE.txt'));
console.log('Bundled helpers built and individually launched. Installer verification remains separate.');
