import {describe,it,expect} from 'vitest';
import {config,study} from './settings';
import {defaultForm,type Media} from './types';
const media=(kind='image',format='png',channels=2)=>({properties:{kind,format,channels}} as Media);
describe('compression choices',()=>{
 it('starts PNG lossless and studies actual independent lossy settings',()=>{expect(config(defaultForm,media()).lossless).toBe(true);expect(study(defaultForm,media()).map(s=>s.quality)).toEqual([80,90,80,65]);expect(study(defaultForm,media()).map(s=>s.lossless)).toEqual([true,false,false,false]);});
 it('retains custom aggressive settings',()=>{expect(study({...defaultForm,mode:'advanced',imageMode:'lossy',sweep:'1, 0, 1'},media('image','webp')).map(s=>s.quality)).toEqual([1,0]);});
 it('uses mono bitrate defaults',()=>{expect(config(defaultForm,media('audio','wav',1)).bitrate).toBe(64);});
 it('supports arbitrary MP3 VBR studies',()=>{expect(study({...defaultForm,mode:'advanced',audioFormat:'mp3',sweep:'0,4,9.9'},media('audio','mp3')).map(s=>s.vbrQuality)).toEqual([0,4,9.9]);});
 it('rejects malformed study input instead of silently dropping values',()=>{expect(()=>study({...defaultForm,mode:'advanced',imageMode:'lossy',sweep:'80,wat'},media())).toThrow();});
});
