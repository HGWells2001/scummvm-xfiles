// SPDX-License-Identifier: GPL-3.0-or-later
// Cache protocol: pinned Shiloh media/pcmcache.rs (MIT; THIRD_PARTY.md).
import {open,readFile,writeFile,mkdir,readdir,realpath,rename,stat,rm} from 'node:fs/promises';
import {resolve,join,dirname,basename,relative,isAbsolute} from 'node:path';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';

export function fnv64(bytes) {
  let hi=0xcbf29ce4, lo=0x84222325;
  for(const byte of bytes) {
    lo=(lo^byte)>>>0;
    const product=lo*435;
    hi=(Math.imul(hi,435)+Math.floor(product/4294967296)+((lo<<8)>>>0))>>>0;
    lo=product>>>0;
  }
  return hi.toString(16).padStart(8,'0')+lo.toString(16).padStart(8,'0');
}
export function hasQdmc(moov) {
  function walk(data,depth) {
    if(depth>8)throw Error('QuickTime nesting limit');
    for(let p=0;p<data.length;) {
      if(data.length-p<8)throw Error('Truncated QuickTime atom');
      const size=data.readUInt32BE(p), type=data.toString('latin1',p+4,p+8);
      if(size<8 || size>data.length-p)throw Error('Invalid QuickTime atom size');
      const body=data.subarray(p+8,p+size);
      if(type==='stsd') {
        if(body.length<8)throw Error('Truncated sample descriptions');
        let q=8;
        for(let i=0;i<body.readUInt32BE(4);i++) {
          if(q+8>body.length)throw Error('Truncated sample entry');
          const n=body.readUInt32BE(q);
          if(n<8 || n>body.length-q)throw Error('Invalid sample entry');
          if(body.toString('latin1',q+4,q+8)==='QDMC')return true;
          q+=n;
        }
      }
      if(['trak','mdia','minf','stbl'].includes(type) && walk(body,depth+1))return true;
      p+=size;
    }
    return false;
  }
  return walk(moov,0);
}
async function qdmcFile(path) {
  const file=await open(path,'r');
  try {
    const total=(await file.stat()).size, header=Buffer.alloc(16);
    for(let p=0;p+8<=total;) {
      if((await file.read(header,0,8,p)).bytesRead!==8)return false;
      let size=header.readUInt32BE(0), head=8;
      if(size===1) {
        if((await file.read(header,8,8,p+8)).bytesRead!==8)return false;
        const wide=header.readBigUInt64BE(8);
        if(wide>BigInt(Number.MAX_SAFE_INTEGER))return false;
        size=Number(wide);head=16;
      }
      if(size===0)size=total-p;
      if(size<head || size>total-p)return false;
      if(header.toString('latin1',4,8)==='moov') {
        if(size-head>32*1024*1024)throw Error('QuickTime header too large');
        const body=Buffer.alloc(size-head);
        if((await file.read(body,0,body.length,p+head)).bytesRead!==body.length)throw Error('Truncated movie header');
        return hasQdmc(body);
      }
      p+=size;
    }
    return false;
  } finally {await file.close();}
}
function run(executable,args) {
  const result=spawnSync(executable,args,{encoding:'utf8',windowsHide:true,maxBuffer:2*1024*1024});
  if(result.error || result.status!==0)throw Error(`${executable}: ${result.error?.message || result.stderr || 'execution failed'}`);
  return result.stdout;
}
export async function prepareAudio(dataDirectory,cacheDirectory,ffmpeg=process.env.XF_FFMPEG || 'ffmpeg',ffprobe=process.env.XF_FFPROBE || 'ffprobe') {
  const data=await realpath(dataDirectory);
  let parent=resolve(cacheDirectory), tail=[];
  while(true){try{parent=await realpath(parent);break;}catch(e){if(e.code!=='ENOENT')throw e;tail.unshift(basename(parent));const next=dirname(parent);if(next===parent)throw e;parent=next;}}
  const cache=join(parent,...tail), rel=relative(data,cache);
  if(rel==='' || (!rel.startsWith('..'+(process.platform==='win32'?'\\':'/')) && rel!=='..' && !isAbsolute(rel)))throw Error('Audio cache must be outside game data');
  const files=[];
  for(const e of await readdir(data,{withFileTypes:true})) {
    if(e.isFile())files.push(join(data,e.name));
    if(e.isDirectory())for(const f of await readdir(join(data,e.name),{withFileTypes:true}))if(f.isFile())files.push(join(data,e.name,f.name));
  }
  let made=0, kept=0, version;
  for(const file of files) {
    if(!/\.(xmv|nmv|amv|dmv|mus|mov)$/i.test(file) || !await qdmcFile(file))continue;
    const key=(basename(dirname(file))+'/'+basename(file)).toLowerCase();
    const source=await readFile(file), hash=fnv64(source);
    const wav=join(cache,'qdmc-v1',key+'.wav'), meta=join(cache,'qdmc-v1',key+'.json');
    try {
      const m=JSON.parse(await readFile(meta,'utf8')), b=await readFile(wav);
      if(m.size===source.length && m.fnv64===hash && m.wav_sha256===createHash('sha256').update(b).digest('hex')) {kept++;continue;}
    } catch(e) {if(!['ENOENT',undefined].includes(e.code))throw e;}
    version ??=run(ffmpeg,['-version']).split('\n')[0];
    const probe=JSON.parse(run(ffprobe,['-v','error','-select_streams','a:0','-show_entries','stream=codec_tag_string,sample_rate,channels','-of','json',file]));
    const stream=probe.streams?.[0];
    if(stream?.codec_tag_string!=='QDMC')throw Error(`QDMC is not the first sound track: ${file}`);
    await mkdir(dirname(wav),{recursive:true});
    const temp=wav+'.'+process.pid+'.tmp';
    try {
      run(ffmpeg,['-nostdin','-v','error','-ignore_editlist','1','-i',file,'-map','0:a:0','-c:a','pcm_s16le','-f','wav','-y',temp]);
      const b=await readFile(temp);
      if(b.length<44 || b.toString('ascii',0,4)!=='RIFF' || b.toString('ascii',8,12)!=='WAVE')throw Error('Decoder produced an invalid WAV');
      // Invalidate the metadata before replacing an entry; readers never accept a partial pair.
      await rm(meta,{force:true});await rename(temp,wav);
      await writeFile(meta,JSON.stringify({source:key,size:source.length,fnv64:hash,rate:Number(stream.sample_rate),channels:stream.channels,decoder:version,wav_sha256:createHash('sha256').update(b).digest('hex')},null,2));
      console.log(`QDMC audio pronto: ${key}`);made++;
    } finally {await rm(temp,{force:true});}
  }
  console.log(`Audio QDMC: ${made} preparati, ${kept} gia pronti. Cache: ${cache}`);
  return {made,kept};
}
if(process.argv[1] && import.meta.url===pathToFileURL(resolve(process.argv[1])).href) {
  if(process.argv.length<4){console.error('Usage: node prepare_audio.mjs DATA CACHE [FFMPEG FFPROBE]');process.exitCode=2;}
  else try{await prepareAudio(...process.argv.slice(2,6));}catch(e){console.error(e.message);process.exitCode=1;}
}
