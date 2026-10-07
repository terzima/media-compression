import path from 'node:path';
const args=process.argv.slice(2), prefix=process.env.MEDIA_CODEC_PREFIX?.replaceAll('\\','/');
if(!prefix)process.exit(1);
const pkg=args.find(x=>['opus','lame','libmp3lame'].includes(x));
if(args.includes('--version')){console.log('1.8.1');process.exit(0);}
if(args.some(x=>x.startsWith('--atleast-pkgconfig-version')))process.exit(0);
if(!pkg)process.exit(1);
if(args.includes('--modversion'))console.log(pkg==='opus'?'1.6.1':'4.0');
else if(args.includes('--cflags'))console.log(`-I${(prefix+'/include')} -I${(prefix+'/include/opus')}`);
else if(args.includes('--libs'))console.log(`-L${(prefix+'/lib')} -l${pkg==='opus'?'opus':'mp3lame'} -lm`);
