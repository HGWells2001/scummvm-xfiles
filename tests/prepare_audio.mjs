// SPDX-License-Identifier: GPL-3.0-or-later
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fnv64,hasQdmc,prepareAudio} from '../tools/prepare_audio.mjs';
function reference(bytes){let h=0xcbf29ce484222325n;for(const b of bytes)h=BigInt.asUintN(64,(h^BigInt(b))*0x100000001b3n);return h.toString(16).padStart(16,'0');}
for(const bytes of [Buffer.alloc(0),Buffer.from('a'),Buffer.from('foobar'),Buffer.from(Array.from({length:10000},(_,i)=>(i*73)&255))])assert.equal(fnv64(bytes),reference(bytes));
function atom(type,body){const b=Buffer.alloc(8+body.length);b.writeUInt32BE(b.length);b.write(type,4,'latin1');body.copy(b,8);return b;}
function movie(codec){const head=Buffer.alloc(8);head.writeUInt32BE(1,4);return atom('trak',atom('mdia',atom('minf',atom('stbl',atom('stsd',Buffer.concat([head,atom(codec,Buffer.alloc(8))]))))));}
assert.equal(hasQdmc(movie('QDMC')),true);
assert.equal(hasQdmc(movie('ima4')),false);
assert.throws(()=>hasQdmc(movie('QDMC').subarray(0,20)),/Invalid|Truncated/);
const malformed=movie('QDMC');malformed.writeUInt32BE(0);assert.throws(()=>hasQdmc(malformed),/Invalid/);
const root=await mkdtemp(join(tmpdir(),'xfiles-audio-'));
try{
 const data=join(root,'data');await mkdir(data);
 await assert.rejects(prepareAudio(data,join(data,'cache')),/outside game data/);
 assert.deepEqual(await prepareAudio(data,join(root,'cache'),'missing-ffmpeg','missing-ffprobe'),{made:0,kept:0});
}finally{await rm(root,{recursive:true,force:true});}
console.log('PASS: QDMC headers, malformed atoms, independent FNV64 and cache isolation');
