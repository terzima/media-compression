import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,readFile,rm,copyFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {prepareRelease,targets} from './prepare-prerelease.mjs';
import {setVersion} from './set-version.mjs';
import publish from './publish-prerelease.cjs';

const version=JSON.parse(await readFile('package.json','utf8')).version,tag=`v${version}`,commit='a'.repeat(40);
const sha=data=>createHash('sha256').update(data).digest('hex');
async function fixture(t){
  const dir=await mkdtemp(path.join(tmpdir(),'compression-release-'));t.after(()=>rm(dir,{recursive:true,force:true}));
  const inputDir=path.join(dir,'candidates');await mkdir(inputDir);
  for(const target of targets){
    const root=path.join(inputDir,`candidate-${target.target}`);await mkdir(root);
    const records=path.join(root,'artifacts');await mkdir(records);
    const bundle=path.join(root,'bundle');await mkdir(bundle);
    await writeFile(path.join(bundle,'THIRD_PARTY_NOTICES.txt'),'bundled copy; use the authoritative artifact record');
    const installer=`Media Compression_${version}_${target.suffix}${target.extension}`,data=Buffer.from(target.target),source=Buffer.from(`source-${target.target}`);
    const item={filename:`${target.extension==='.exe'?'nsis':'dmg'}/${installer}`,bytes:data.length,sha256:sha(data)};
    const values={
      'build-evidence.json':{schemaVersion:1,commit,workingTreeChanges:false,target:target.target,qualification:'unsigned development candidate',artifacts:[item]},
      'installed-bundle-evidence.json':{target:target.target,installer:{sha256:item.sha256},checks:['Installed agent passed checks','Installed helpers passed checks','Installed desktop remained running during smoke']},
      'native-dependencies.json':{target:target.target,evidence:Array(8).fill({})},
      'distribution-review.json':{schemaVersion:1,rust:[{}],native:Array(10).fill({}),ffmpegFlags:['--disable-gpl','--disable-nonfree']},
      'dependency-manifest.json':{},'webview2-provenance.json':{signatureStatus:'Unavailable'},
    };
    for(const [file,value]of Object.entries(values))await writeFile(path.join(records,file),JSON.stringify(value));
    await writeFile(path.join(root,installer),data);await writeFile(path.join(records,'dependency-source.tar.gz'),source);
    await writeFile(path.join(records,'installer-checksums.sha256'),`${item.sha256}  ${item.filename}\n`);
    await writeFile(path.join(records,'dependency-source.sha256'),`${sha(source)}  dependency-source.tar.gz\n`);
    await writeFile(path.join(records,'THIRD_PARTY_NOTICES.txt'),'fixture license notice');
  }
  const sourceFile=path.join(dir,'project-source.tar.gz');await writeFile(sourceFile,'test source');
  return {dir,inputDir,outputDir:path.join(dir,'release'),sourceFile,tag,commit,runUrl:'https://github.com/terzima/media-compression/actions/runs/1'};
}
async function change(input,target,file,fn){const p=path.join(input,`candidate-${target}`,'artifacts',file);const v=JSON.parse(await readFile(p,'utf8'));fn(v);await writeFile(p,JSON.stringify(v));}
test('stage all three targets and verify every renamed public asset against SHA256SUMS',async t=>{
  const f=await fixture(t),manifest=await prepareRelease(f);assert.equal(manifest.platforms.length,3);
  const sums=(await readFile(path.join(f.outputDir,'SHA256SUMS'),'utf8')).trim().split('\n');
  for(const line of sums){const [digest,name]=line.split('  ');assert.equal(sha(await readFile(path.join(f.outputDir,name))),digest);}
  assert.ok(!(await readFile(path.join(f.outputDir,'RELEASE_NOTES.md'),'utf8')).includes('<COMMIT>'));
});
test('reject changed installer or dependency source before staging',async t=>{
  for(const source of [false,true]){
    const f=await fixture(t),p=targets[0];
    await writeFile(path.join(f.inputDir,`candidate-${p.target}`,source?'artifacts/dependency-source.tar.gz':`Media Compression_${version}_${p.suffix}${p.extension}`),'tampered');
    await assert.rejects(prepareRelease(f),/checksum/);
    await assert.rejects(readFile(path.join(f.outputDir,'SHA256SUMS')),/ENOENT/);
  }
});
test('reject wrong commit, dirty tree, missing installed-agent evidence and missing platform',async t=>{
  for(const mode of ['commit','dirty','agent','platform']){
    const f=await fixture(t),target=targets[0].target;
    if(mode==='commit')await change(f.inputDir,target,'build-evidence.json',v=>v.commit='b'.repeat(40));
    if(mode==='dirty')await change(f.inputDir,target,'build-evidence.json',v=>v.workingTreeChanges=true);
    if(mode==='agent')await change(f.inputDir,target,'installed-bundle-evidence.json',v=>v.checks=[]);
    if(mode==='platform')await rm(path.join(f.inputDir,`candidate-${target}`),{recursive:true});
    await assert.rejects(prepareRelease(f));
  }
});
test('unsigned publication rejects stable tags and version mismatches',async t=>{
  const f=await fixture(t);await assert.rejects(prepareRelease({...f,tag:'v0.1.0'}),/Only explicit alpha/);
  await assert.rejects(prepareRelease({...f,tag:'v999.999.999-alpha.999'}),/versions differ/);
});
test('version helper updates all manifests and project lock entries',async t=>{
  const f=await fixture(t),root=path.join(f.dir,'version');await mkdir(path.join(root,'src-tauri'),{recursive:true});
  for(const file of ['package.json','package-lock.json','src-tauri/tauri.conf.json','Cargo.toml','Cargo.lock'])await copyFile(file,path.join(root,file));
  await setVersion('0.1.0-alpha.2',root);
  assert.equal(JSON.parse(await readFile(path.join(root,'package-lock.json'))).packages[''].version,'0.1.0-alpha.2');
  assert.ok((await readFile(path.join(root,'Cargo.lock'),'utf8')).includes('name = "media-agent"\nversion = "0.1.0-alpha.2"'));
  await assert.rejects(setVersion('bad; command',root),/Use x.y.z/);
});
function client({failUpload=false}={}){
  const state={release:null,assets:[],published:false};
  const repos={
    getReleaseByTag:async()=>{if(!state.release)throw Object.assign(Error('missing'),{status:404});return{data:state.release};},
    createRelease:async args=>{state.release={...args,id:1};return{data:state.release};},
    listReleaseAssets:async()=>{},
    uploadReleaseAsset:async args=>{if(failUpload)throw Error('upload interrupted');const a={name:args.name,size:args.data.length,digest:`sha256:${sha(args.data)}`};state.assets.push(a);return{data:a};},
    updateRelease:async args=>{state.published=true;state.release={...state.release,...args,html_url:'https://example.test/release'};return{data:state.release};},
  };
  return{state,github:{rest:{repos},paginate:async()=>state.assets},context:{repo:{owner:'terzima',repo:'media-compression'},eventName:'push',ref:`refs/tags/${tag}`,sha:commit},core:{notice:()=>{},summary:{addLink(){return this;},write:async()=>{}}}};
}
test('publish only after complete uploads; reject overwriting an existing published release',async t=>{
  const f=await fixture(t);await prepareRelease(f);const c=client();
  // Publisher resolves paths without modifying global cwd; fixtures are isolated.
  const old=process.env.MEDIA_RELEASE_DIRECTORY;process.env.MEDIA_RELEASE_DIRECTORY=f.outputDir;t.after(()=>{if(old===undefined)delete process.env.MEDIA_RELEASE_DIRECTORY;else process.env.MEDIA_RELEASE_DIRECTORY=old;});
  await publish(c);assert.equal(c.state.published,true);assert.equal(c.state.release.prerelease,true);
  await assert.rejects(publish(c),/immutable/);
});
test('interrupted upload leaves a draft, and altered staged assets never create a release',async t=>{
  const f=await fixture(t);await prepareRelease(f);process.env.MEDIA_RELEASE_DIRECTORY=f.outputDir;t.after(()=>delete process.env.MEDIA_RELEASE_DIRECTORY);
  const c=client({failUpload:true});await assert.rejects(publish(c),/interrupted/);assert.equal(c.state.published,false);assert.equal(c.state.release.draft,true);
  await writeFile(path.join(f.outputDir,'RELEASE_NOTES.md'),'changed');const clean=client();await assert.rejects(publish(clean),/changed/);assert.equal(clean.state.release,null);
});
