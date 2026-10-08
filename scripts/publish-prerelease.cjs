const fs=require('node:fs/promises');
const crypto=require('node:crypto');
const path=require('node:path');

module.exports=async({github,context,core})=>{
  const dir=path.resolve(process.env.MEDIA_RELEASE_DIRECTORY||'artifacts/release');
  const manifest=JSON.parse(await fs.readFile(path.join(dir,'release-manifest.json'),'utf8'));
  if(context.eventName!=='push'||context.ref!==`refs/tags/${manifest.tag}`||context.sha!==manifest.commit||!/^v\d+\.\d+\.\d+-alpha\.[1-9]\d*$/.test(manifest.tag))throw Error('Publication must come from the matching alpha tag push');
  const names=(await fs.readdir(dir)).sort();
  const sums=(await fs.readFile(path.join(dir,'SHA256SUMS'),'utf8')).trim().split('\n');
  const expected=new Map();
  for(const line of sums){const match=/^([a-f0-9]{64})  ([A-Za-z0-9_.-]+)$/.exec(line);if(!match||expected.has(match[2]))throw Error('Invalid release checksum list');expected.set(match[2],match[1]);}
  if(names.length!==expected.size+1||names.some(n=>n!=='SHA256SUMS'&&!expected.has(n)))throw Error('Release file list differs from checksums');
  for(const [name,sha]of expected)if(crypto.createHash('sha256').update(await fs.readFile(path.join(dir,name))).digest('hex')!==sha)throw Error(`Release file changed: ${name}`);
  const body=await fs.readFile(path.join(dir,'RELEASE_NOTES.md'),'utf8');
  let release;
  try{release=(await github.rest.repos.getReleaseByTag({...context.repo,tag:manifest.tag})).data;}catch(e){if(e.status!==404)throw e;}
  if(release){
    if(!release.draft)throw Error('Published releases are immutable here; use a new version');
    if(release.target_commitish!==manifest.commit||release.body!==body||!release.prerelease)throw Error('Existing draft belongs to different release material');
  }else release=(await github.rest.repos.createRelease({...context.repo,tag_name:manifest.tag,target_commitish:manifest.commit,name:`Media Compression ${manifest.tag} — unsigned preview`,body,draft:true,prerelease:true,make_latest:'false'})).data;
  const existing=await github.paginate(github.rest.repos.listReleaseAssets,{...context.repo,release_id:release.id,per_page:100});
  for(const name of names){
    const data=await fs.readFile(path.join(dir,name)),sha=crypto.createHash('sha256').update(data).digest('hex');
    const found=existing.find(a=>a.name===name);
    if(found){if(found.size!==data.length||found.digest!==`sha256:${sha}`)throw Error(`Existing draft asset differs: ${name}`);continue;}
    const uploaded=(await github.rest.repos.uploadReleaseAsset({...context.repo,release_id:release.id,name,data,headers:{'content-type':'application/octet-stream','content-length':data.length}})).data;
    if(uploaded.size!==data.length||uploaded.digest!==`sha256:${sha}`)throw Error(`Uploaded asset hash/size mismatch: ${name}`);
  }
  const uploaded=await github.paginate(github.rest.repos.listReleaseAssets,{...context.repo,release_id:release.id,per_page:100});
  if(uploaded.length!==names.length||uploaded.some(a=>!names.includes(a.name)))throw Error('Draft assets incomplete or unexpected');
  const published=(await github.rest.repos.updateRelease({...context.repo,release_id:release.id,draft:false,prerelease:true,make_latest:'false'})).data;
  core.notice(`Published ${published.html_url}`);
  await core.summary.addLink('Published alpha preview',published.html_url).write();
};
