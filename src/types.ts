export interface Settings { format:string; lossless:boolean; quality:number|null; bitrate:number|null; vbrQuality:number|null; effort:number|null; background:string|null }
export interface Properties {kind:'image'|'audio';format:string;width:number|null;height:number|null;alpha:boolean;bitDepth:number|null;sampleRate:number|null;channels:number|null;duration:number|null;sampleFormat:string|null;colorProfile?:string|null;channelLayout?:string|null}
export interface Diagnostics {ssimLight:number|null;ssimDark:number|null;alphaMaxError:number|null;alphaMeanError:number|null;pixelIdentical:boolean|null;durationDelta:number|null;notices:string[]}
export interface Candidate {id:string;mediaId:string;bytes:number;sha256:string;settings:Settings[];properties:Properties;diagnostics:Diagnostics;preview:string|null;exported:string[];exportErrors?:string[]}
export interface Media {id:string;name:string;relativeName:string;bytes:number;sha256:string;properties:Properties|null;preview:string|null;error:string|null;candidates:Candidate[]}
export interface Job {id:string;mediaId:string;state:string;stage:string;completed:number;total:number;errors:string[]}
export interface Snapshot {media:Media[];jobs:Job[];tools:string[]}
export interface Playback {id:string|null;position:number;duration:number;paused:boolean;volume:number;error:string|null}
export interface Form {mode:'quick'|'advanced';preset:number;imageFormat:string;audioFormat:string;imageMode:string;quality:number;bitrate:number;vbrQuality:number;useVbr:boolean;effort:number;background:string;flatten:boolean;sweep:string}
export const defaultForm:Form={mode:'quick',preset:1,imageFormat:'same',audioFormat:'aac',imageMode:'auto',quality:80,bitrate:128,vbrQuality:4,useVbr:true,effort:2,background:'#ffffff',flatten:false,sweep:'90,80,65'};
