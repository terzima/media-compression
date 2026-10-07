import {defaultForm,type Form,type Media,type Settings} from './types';
export function config(form:Form,media:Media,index=form.preset):Settings {
 const p=media.properties;if(!p)throw new Error('Unsupported input');
 const advanced=form.mode==='advanced';const image=p.kind==='image';
 const format=image?(form.imageFormat==='same'?p.format:form.imageFormat):form.audioFormat;
 const lossless=image?(form.imageMode==='auto'?format==='png':form.imageMode==='lossless'):format==='flac';
 const mono=p.channels===1;
 return {format,lossless,quality:image?(advanced?form.quality:[90,80,65][index]):null,
  bitrate:!image&&format!=='flac'&&!(format==='mp3'&&(!advanced||form.useVbr))?(advanced?form.bitrate:(format==='opus'?(mono?[64,48,32]:[160,96,64]):(mono?[96,64,48]:[192,128,96]))[index]):null,
  vbrQuality:!image&&format==='mp3'&&(!advanced||form.useVbr)?(advanced?form.vbrQuality:[2,4,6][index]):null,
  effort:format==='jpeg'||!image&&format==='aac'?null:advanced?form.effort:format==='flac'||format==='mp3'?5:format==='opus'?10:format==='png'&&!lossless?3:2,
  background:image&&form.flatten?form.background:null};
}
export function study(form:Form,media:Media):Settings[] {
 if(form.mode==='quick'){
  if(media.properties?.kind==='image'){
   if((media.properties.bitDepth??8)>8&&config(form,media).format==='png')return [{...config(form,media),lossless:true}];
   const baseline=config(form,media);const results=[0,1,2].map(i=>({...config(form,media,i),lossless:false}));
   return baseline.format==='png'||baseline.format==='webp'?[{...baseline,lossless:true},...results]:results;
  }
  return form.audioFormat==='flac'?[config(form,media)]:[0,1,2].map(i=>config(form,media,i));
 }
 const base=config(form,media);if(base.lossless)return [base];
 const tokens=form.sweep.split(/[\s,;]+/).filter(Boolean);
 if(!tokens.length||tokens.length>512)throw new Error('Enter 1–512 study settings, separated by commas');
 if(tokens.some(v=>!/^\d+(\.\d+)?$/.test(v)))throw new Error('Study settings must be numbers, separated by commas');
 const values=[...new Set(tokens.map(Number))];if(values.some(v=>!Number.isFinite(v)))throw new Error('Invalid study settings');
 return values.map(value=>media.properties?.kind==='image'?{...base,quality:value}:base.vbrQuality!==null?{...base,vbrQuality:value}:{...base,bitrate:value});
}
export function loadForm():Form {try {const stored=JSON.parse(localStorage.getItem('media-compression-settings-v1')||'{}');return {...defaultForm,...stored};}catch{return defaultForm;}}
export const bytes=(n:number)=>n<1000?`${n} B`:n<1e6?`${(n/1000).toFixed(1)} KB`:`${(n/1e6).toFixed(2)} MB`;
export const savings=(source:number,candidate:number)=>source?100*(1-candidate/source):0;
export const label=(s:Settings)=>s.lossless?`${s.format.toUpperCase()} · lossless`:`${s.format.toUpperCase()} · ${s.quality!==null?`quality ${s.quality}`:s.vbrQuality!==null?`VBR ${s.vbrQuality}`:`${s.bitrate} kbps`}`;
