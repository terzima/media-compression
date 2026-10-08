import {readFile,writeFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import path from 'node:path';

export async function setVersion(version,root=process.cwd()) {
  if(!/^\d+\.\d+\.\d+(?:-alpha\.[1-9]\d*)?$/.test(version))throw Error('Use x.y.z or x.y.z-alpha.N');
  const files=new Map();
  for(const file of ['package.json','package-lock.json','src-tauri/tauri.conf.json']){
    const value=JSON.parse(await readFile(path.join(root,file),'utf8'));
    value.version=version;
    if(file==='package-lock.json')value.packages[''].version=version;
    files.set(file,JSON.stringify(value,null,2)+'\n');
  }
  const cargo=await readFile(path.join(root,'Cargo.toml'),'utf8');
  if(!/\[workspace.package\]\nversion = "[^"]+"/.test(cargo))throw Error('Workspace version missing');
  files.set('Cargo.toml',cargo.replace(/(\[workspace.package\]\nversion = ")[^"]+/,`$1${version}`));
  let lock=await readFile(path.join(root,'Cargo.lock'),'utf8');
  for(const name of ['media-agent','media-compression','media-engine']){
    const expression=new RegExp(`(name = "${name}"\\nversion = ")[^"]+`);
    if(!expression.test(lock))throw Error(`Lockfile package missing: ${name}`);
    lock=lock.replace(expression,`$1${version}`);
  }
  files.set('Cargo.lock',lock);
  for(const [file,data]of files)await writeFile(path.join(root,file),data);
}
if(process.argv[1]&&import.meta.url===pathToFileURL(path.resolve(process.argv[1])).href){
  await setVersion(process.argv[2]);console.log(`Version set to ${process.argv[2]}`);
}
