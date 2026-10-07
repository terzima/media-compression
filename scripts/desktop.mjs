import {spawnSync} from 'node:child_process';import {homedir} from 'node:os';import path from 'node:path';
const env={...process.env,CI:process.env.CI||'true',PATH:[path.join(homedir(),'.cargo/bin'),process.env.PATH].join(path.delimiter)};
for(const args of [['scripts/build-worker.mjs'],['node_modules/@tauri-apps/cli/tauri.js',...process.argv.slice(2)]]){const p=spawnSync(process.execPath,args,{env,stdio:'inherit'});if(p.status!==0)process.exit(p.status||1);}
