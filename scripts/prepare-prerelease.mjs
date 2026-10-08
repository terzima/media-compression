import {readFile,writeFile,readdir,mkdir,copyFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {pathToFileURL} from 'node:url';
import path from 'node:path';

export const targets=[
  {target:'aarch64-apple-darwin',label:'Apple Silicon Mac',suffix:'macos-arm64',extension:'.dmg'},
  {target:'x86_64-apple-darwin',label:'Intel Mac',suffix:'macos-x64',extension:'.dmg'},
  {target:'x86_64-pc-windows-msvc',label:'Windows x64',suffix:'windows-x64',extension:'.exe'},
];
const hash=data=>createHash('sha256').update(data).digest('hex');
async function walk(root){
  const files=[];
  for(const item of await readdir(root,{withFileTypes:true})){
    const file=path.join(root,item.name);
    if(item.isSymbolicLink())throw Error('Release input contains a symlink');
    if(item.isDirectory())files.push(...await walk(file));else if(item.isFile())files.push(file);
  }
  return files;
}
function unique(files,name){
  const found=files.filter(file=>path.basename(file)===name);
  if(found.length!==1)throw Error(`Expected one ${name}, found ${found.length}`);
  return found[0];
}
const record=(files,name)=>unique(files.filter(file=>path.basename(path.dirname(file))==='artifacts'),name);
const json=async file=>JSON.parse(await readFile(file,'utf8'));
export async function prepareRelease({inputDir,outputDir,tag,commit,runUrl,sourceFile,notesFile,root=process.cwd()}){
  if(!/^v\d+\.\d+\.\d+-alpha\.[1-9]\d*$/.test(tag))throw Error('Only explicit alpha tags may publish unsigned previews');
  if(!/^[a-f0-9]{40}$/.test(commit))throw Error('Exact commit required');
  const version=tag.slice(1),pkg=await json(path.join(root,'package.json'));
  const tauri=await json(path.join(root,'src-tauri/tauri.conf.json'));
  const lock=await json(path.join(root,'package-lock.json'));
  const cargo=await readFile(path.join(root,'Cargo.toml'),'utf8');
  const cargoLock=await readFile(path.join(root,'Cargo.lock'),'utf8');
  if([pkg.version,tauri.version,lock.version,lock.packages[''].version,cargo.match(/\[workspace.package\]\nversion = "([^"]+)"/)?.[1]].some(v=>v!==version))throw Error('Tag and application versions differ');
  for(const name of ['media-agent','media-compression','media-engine'])if(!cargoLock.includes(`name = "${name}"\nversion = "${version}"`))throw Error('Rust lockfile version differs');
  const pending=[],platforms=[];
  for(const platform of targets){
    const files=await walk(path.join(inputDir,`candidate-${platform.target}`));
    const build=await json(record(files,'build-evidence.json'));
    if(build.commit!==commit||build.workingTreeChanges!==false||build.target!==platform.target||build.qualification!=='unsigned development candidate')throw Error(`Build provenance mismatch: ${platform.target}`);
    if(build.artifacts.length!==1)throw Error('Expected exactly one platform installer');
    const item=build.artifacts[0];
    if(!/^(dmg|nsis)\/[^/\\]+$/.test(item.filename)||!item.filename.endsWith(platform.extension)||!item.filename.includes(version))throw Error('Unexpected installer filename/version');
    const installer=unique(files,path.posix.basename(item.filename));
    const data=await readFile(installer);
    if(hash(data)!==item.sha256||data.length!==item.bytes)throw Error('Installer checksum/size mismatch');
    const checksums=await readFile(record(files,'installer-checksums.sha256'),'utf8');
    if(!checksums.split(/\r?\n/).includes(`${item.sha256}  ${item.filename}`))throw Error('Installer checksum record mismatch');
    const installed=await json(record(files,'installed-bundle-evidence.json'));
    if(installed.target!==platform.target||installed.installer.sha256!==item.sha256||!installed.checks.some(c=>c.startsWith('Installed agent passed'))||!installed.checks.some(c=>c.startsWith('Installed helpers passed'))||!installed.checks.some(c=>c.startsWith('Installed desktop remained running')))throw Error('Installed runtime evidence incomplete');
    const audit=await json(record(files,'native-dependencies.json'));
    if(audit.target!==platform.target||audit.evidence.length!==8)throw Error('Native dependency evidence incomplete');
    const review=await json(record(files,'distribution-review.json'));
    if(review.schemaVersion!==1||!review.rust.length||review.native.length!==10||!review.ffmpegFlags.includes('--disable-gpl')||!review.ffmpegFlags.includes('--disable-nonfree'))throw Error('Distribution material review incomplete');
    const source=record(files,'dependency-source.tar.gz'),sourceHash=hash(await readFile(source));
    const sourceChecksum=(await readFile(record(files,'dependency-source.sha256'),'utf8')).trim();
    if(sourceChecksum!==`${sourceHash}  dependency-source.tar.gz`)throw Error('Dependency source checksum mismatch');
    const installerName=`Media-Compression_${version}_${platform.suffix}${platform.extension}`;
    pending.push({file:installer,name:installerName},
      {file:source,name:`media-compression_${version}_dependencies_${platform.target}.tar.gz`});
    for(const name of ['THIRD_PARTY_NOTICES.txt','dependency-manifest.json','distribution-review.json','build-evidence.json','installed-bundle-evidence.json','native-dependencies.json'])pending.push({file:record(files,name),name:`${platform.target}_${name}`});
    if(platform.extension==='.exe')pending.push({file:record(files,'webview2-provenance.json'),name:`${platform.target}_webview2-provenance.json`});
    platforms.push({...platform,installer:installerName,sha256:item.sha256,bytes:item.bytes,buildCommit:build.commit});
  }
  // Validate every input before staging anything. Never reuse/overwrite a release folder.
  await mkdir(outputDir);
  const assets=[];
  for(const item of pending){await copyFile(item.file,path.join(outputDir,item.name));const data=await readFile(item.file);assets.push({name:item.name,bytes:data.length,sha256:hash(data)});}
  const sourceName=`media-compression_${version}_source.tar.gz`;
  if(sourceFile)await copyFile(sourceFile,path.join(outputDir,sourceName));
  else{
    const result=spawnSync('git',['archive','--format=tar.gz',`--prefix=media-compression-${version}/`,`--output=${path.resolve(outputDir,sourceName)}`,commit],{cwd:root,encoding:'utf8'});
    if(result.status!==0)throw Error(result.stderr||'Project source archive failed');
  }
  const projectSource=await readFile(path.join(outputDir,sourceName));assets.push({name:sourceName,bytes:projectSource.length,sha256:hash(projectSource)});
  const table=platforms.map(p=>`| ${p.label} | ${p.installer} |`).join('\n');
  const notes=(await readFile(notesFile||path.join(root,'docs/RELEASE_NOTES.md'),'utf8')).replaceAll('<VERSION>',version).replaceAll('<COMMIT>',commit).replaceAll('<RUN_URL>',runUrl).replaceAll('<ASSETS_TABLE>',table);
  await writeFile(path.join(outputDir,'RELEASE_NOTES.md'),notes);assets.push({name:'RELEASE_NOTES.md',bytes:Buffer.byteLength(notes),sha256:hash(notes)});
  const manifest={schemaVersion:1,tag,commit,runUrl,qualification:'unsigned alpha preview; not stable/minimum-OS qualification',platforms,assets};
  const manifestText=JSON.stringify(manifest,null,2)+'\n';await writeFile(path.join(outputDir,'release-manifest.json'),manifestText);
  await writeFile(path.join(outputDir,'SHA256SUMS'),[...assets,{name:'release-manifest.json',sha256:hash(manifestText)}].map(a=>`${a.sha256}  ${a.name}`).join('\n')+'\n');
  return manifest;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(path.resolve(process.argv[1])).href){
  const manifest=await prepareRelease({inputDir:'artifacts/candidates',outputDir:'artifacts/release',tag:process.env.GITHUB_REF_NAME,commit:process.env.GITHUB_SHA,runUrl:`${process.env.GITHUB_SERVER_URL}/${process.env.GITHUB_REPOSITORY}/actions/runs/${process.env.GITHUB_RUN_ID}`});
  console.log(`Verified and staged ${manifest.tag} for all three targets`);
}
