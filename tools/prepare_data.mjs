// SPDX-License-Identifier: GPL-3.0-or-later
// ISO9660 data-file extractor. Associated/resource-fork records (flag 0x04) are skipped.
// Only writes to a separate destination; original folders and disc images are read-only.
import {open,mkdir,stat,lstat,realpath,rename,writeFile} from 'node:fs/promises';
import {resolve,join,basename,extname} from 'node:path';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';

export function safeName(name){
  return !!name && name!=='.' && name!=='..' && !/[\\/:\x00-\x1f]/.test(name) && !/[ .]$/.test(name);
}
export function records(buffer){
  const result=[];
  for(let p=0;p<buffer.length;){
    const n=buffer[p];
    if(!n){p=(Math.floor(p/2048)+1)*2048;continue;}
    if(n<34 || p+n>buffer.length || (p%2048)+n>2048)throw Error('Invalid ISO directory record');
    const r=buffer.subarray(p,p+n), length=r[32];
    if(length>n-33)throw Error('Invalid ISO filename length');
    p+=n;
    if(length===1 && r[33]<=1)continue;
    const name=r.toString('ascii',33,33+length).replace(/;\d+$/,'').replace(/\.$/,'').toUpperCase();
    if(!safeName(name))throw Error('Unsafe ISO filename');
    if(r.readUInt32LE(2)!==r.readUInt32BE(6) || r.readUInt32LE(10)!==r.readUInt32BE(14))throw Error('ISO extent byte-order mismatch');
    result.push({name,extent:r.readUInt32LE(2),size:r.readUInt32LE(10),flags:r[25],directory:!!(r[25]&2)});
  }
  return result;
}
async function readExact(file,offset,length,total){
  if(!Number.isSafeInteger(offset) || !Number.isSafeInteger(length) || offset<0 || length<0 || offset+length>total)throw Error('ISO extent outside image');
  const b=Buffer.alloc(length);let done=0;
  while(done<length){const r=await file.read(b,done,length-done,offset+done);if(!r.bytesRead)throw Error('Truncated ISO');done+=r.bytesRead;}
  return b;
}
async function digest(file,offset,size,total){
  const h=createHash('sha256');
  for(let p=0;p<size;p+=1048576)h.update(await readExact(file,offset+p,Math.min(1048576,size-p),total));
  return h.digest('hex');
}
async function ensureDirectory(root,relative){
  let current=root;
  for(const part of relative.split('/').filter(Boolean)){
    if(!safeName(part))throw Error('Unsafe output directory');
    current=join(current,part);
    await mkdir(current,{recursive:true});
    const s=await lstat(current);
    if(!s.isDirectory() || s.isSymbolicLink())throw Error('Output directory must not be a symlink');
  }
}
const directories=new Set(['MININST','MEDINST','XG','XV','XN','XS','XT']);
const extensions=new Set(['.PFF','.NMV','.XMV','.AMV','.DMV','.HOT','.MUS','.XTX','.TTR','.HDB','.GAM']);
export function quicktimeIdentity(input){
  // Duplicates on different discs may change creation dates and unused 'free' bytes.
  // Accept them only if every other atom byte, including sample tables and mdat, is identical.
  const b=Buffer.from(input), hash=createHash('sha256');let movies=0,media=0;
  const containers=new Set(['moov','trak','mdia','minf','stbl','dinf','edts']);
  function walk(start,end,depth){
    if(depth>16)return false;
    for(let p=start;p<end;){
      if(end-p<8)return false;
      let n=b.readUInt32BE(p);const type=b.toString('ascii',p+4,p+8);
      if(n===0)n=end-p;
      if(n<8 || n>end-p)return false;
      if(type==='moov')++movies;if(type==='mdat')++media;
      if(type!=='free' && type!=='wide'){
        hash.update(b.subarray(p,p+8));
        if(containers.has(type)){if(!walk(p+8,p+n,depth+1))return false;}
        else {
          if(['mvhd','tkhd','mdhd'].includes(type)){
            const v=b[p+8], times=v===0?8:v===1?16:0;
            if(!times || n<12+times)return false;
            b.fill(0,p+12,p+12+times);
          }
          hash.update(b.subarray(p+8,p+n));
        }
      }
      p+=n;
    }
    return true;
  }
  return walk(0,b.length,0) && movies && media ? hash.digest('hex') : null;
}
export async function prepareData(images,destination,progress=()=>{}){
  if(!images.length)throw Error('Supply at least one original ISO');
  await mkdir(destination,{recursive:true});
  if((await lstat(destination)).isSymbolicLink())throw Error('Destination must not be a symlink');
  const output=await realpath(destination), manifest=[];
  let associatedSkipped=0,copied=0,reused=0,bytes=0,equivalentMovies=0;
  for(const image of images){
    const iso=await open(image,'r');
    try{
      const total=(await iso.stat()).size;
      const pvd=await readExact(iso,16*2048,2048,total);
      if(pvd[0]!==1 || pvd.toString('ascii',1,6)!=='CD001' || pvd.readUInt16LE(128)!==2048)throw Error('Expected ISO9660 image with 2048-byte sectors');
      const root={extent:pvd.readUInt32LE(158),size:pvd.readUInt32LE(166)};
      const seen=new Set();
      async function walk(dir,path,depth){
        if(depth>8 || dir.size>16*1024*1024 || seen.has(dir.extent))throw Error('Invalid ISO directory tree');
        seen.add(dir.extent);
        for(const record of records(await readExact(iso,dir.extent*2048,dir.size,total))){
          if(record.flags&4){++associatedSkipped;continue;}
          if(record.flags&128)throw Error('Multi-extent ISO file is not supported');
          const components=[...path,record.name];
          if(record.directory){if(directories.has(record.name))await walk(record,components,depth+1);continue;}
          const extension=extname(record.name);
          if(!extensions.has(extension) && !/^XFILES[^/]*\.(EXE|DLL)$/.test(record.name))continue;
          // Installation-specific directories are merged into the standard installed-game layout.
          const parts=components.filter(p=>p!=='MININST' && p!=='MEDINST');
          const relative=parts.join('/'), target=join(output,...parts), parent=parts.slice(0,-1).join('/');
          await ensureDirectory(output,parent);
          const hash=await digest(iso,record.extent*2048,record.size,total);let writtenHash=hash;
          let exists=false;
          try{const s=await lstat(target);if(s.isSymbolicLink() || !s.isFile())throw Error('Unsafe existing output');exists=true;
            const old=await open(target,'r');try{
              writtenHash=await digest(old,0,s.size,s.size);
              if(writtenHash!==hash){
                if(!['.XMV','.NMV','.AMV','.DMV'].includes(extension) || s.size>128*1024*1024 || record.size>128*1024*1024)throw Error('Conflicting existing output: '+relative);
                const before=quicktimeIdentity(await readExact(old,0,s.size,s.size));
                const incoming=quicktimeIdentity(await readExact(iso,record.extent*2048,record.size,total));
                if(!before || before!==incoming)throw Error('Conflicting existing output: '+relative);
                ++equivalentMovies;
              }
            }finally{await old.close();}
          }catch(e){if(e.code!=='ENOENT')throw e;}
          if(exists){++reused;}else{
            const temp=target+'.partial', dest=await open(temp,'wx');
            try{for(let p=0;p<record.size;p+=1048576){const b=await readExact(iso,record.extent*2048+p,Math.min(1048576,record.size-p),total);let n=0;while(n<b.length){const w=await dest.write(b,n,b.length-n);if(!w.bytesWritten)throw Error('Short output write');n+=w.bytesWritten;}}}finally{await dest.close();}
            await rename(temp,target);++copied;bytes+=record.size;
          }
          manifest.push({source:basename(image),path:relative,size:record.size,sha256:writtenHash,source_sha256:hash});
        }
        seen.delete(dir.extent);
      }
      await walk(root,[],0);
      progress({disc:basename(image),copied,reused,associatedSkipped,equivalentMovies,bytes});
    }finally{await iso.close();}
  }
  const manifestTarget=join(output,'PREPARATION_MANIFEST.json');
  try{if((await lstat(manifestTarget)).isSymbolicLink())throw Error('Manifest must not be a symlink');}catch(e){if(e.code!=='ENOENT')throw e;}
  await writeFile(manifestTarget,JSON.stringify({images:images.map(x=>basename(x)),copied,reused,associatedSkipped,equivalentMovies,bytes,files:manifest},null,2));
  return {output,copied,reused,associatedSkipped,equivalentMovies,bytes};
}
if(typeof process!=='undefined' && process.argv[1] && import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const [destination,...images]=process.argv.slice(2);
  if(!destination || !images.length){console.error('Usage: node prepare_data.mjs OUTPUT_DIRECTORY DISC1.iso [DISC2.iso ...]');process.exitCode=2;}
  else prepareData(images,destination,s=>console.log(JSON.stringify(s))).then(s=>console.log(JSON.stringify(s))).catch(e=>{console.error(e.message);process.exitCode=1;});
}
