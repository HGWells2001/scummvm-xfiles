// SPDX-License-Identifier: GPL-3.0-or-later
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,readFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {prepareData,records,safeName,quicktimeIdentity} from '../tools/prepare_data.mjs';
function atom(type,data){const b=Buffer.alloc(8+data.length);b.writeUInt32BE(b.length);b.write(type,4,'ascii');data.copy(b,8);return b;}
function entry(name,extent,size,flags=0){
  const n=33+name.length+((33+name.length)%2), b=Buffer.alloc(n);
  b[0]=n;b.writeUInt32LE(extent,2);b.writeUInt32BE(extent,6);b.writeUInt32LE(size,10);b.writeUInt32BE(size,14);b[25]=flags;b[32]=name.length;b.write(name,33,'ascii');return b;
}
export async function run(){
  for(const name of ['../x','a/b','x\\y','C:x','..','x.','x '])assert.equal(safeName(name),false);
  assert.equal(safeName('X.PFF'),true);
  const r=entry('X.PFF',22,32);assert.equal(records(r)[0].size,32);
  const bad=Buffer.from(r);bad[14]^=1;assert.throws(()=>records(bad),/byte-order/);
  assert.throws(()=>records(Buffer.from([3,0,0])),/directory/);
  const iso=Buffer.alloc(24*2048), pvd=iso.subarray(16*2048,17*2048);
  pvd[0]=1;pvd.write('CD001',1);pvd[6]=1;pvd.writeUInt16LE(2048,128);pvd.writeUInt16BE(2048,130);
  const root=entry(String.fromCharCode(0),20,2048,2);root.copy(pvd,156);
  Buffer.concat([entry('X.PFF',21,286,4),entry('X.PFF',22,32)]).copy(iso,20*2048);
  const actual=Buffer.alloc(32,7);actual.write('PFF ');actual.copy(iso,22*2048);
  const temp=await mkdtemp(join(tmpdir(),'xfiles-iso-test-')), image=join(temp,'fixture.iso'), out=join(temp,'data');
  await writeFile(image,iso);
  const first=await prepareData([image],out);
  assert.equal(first.copied,1);assert.equal(first.associatedSkipped,1);
  assert.deepEqual(await readFile(join(out,'X.PFF')),actual);
  const again=await prepareData([image],out);assert.equal(again.reused,1);assert.equal(again.copied,0);
  await writeFile(join(out,'X.PFF'),'different');
  await assert.rejects(prepareData([image],out),/Conflicting/);
  assert.equal((await readFile(join(out,'X.PFF'))).toString(),'different');
  assert.deepEqual(await readFile(image),iso);
  const header=Buffer.alloc(100), media=Buffer.from('synthetic compressed media');
  const movie=Buffer.concat([atom('moov',atom('mvhd',header)),atom('free',Buffer.alloc(32,1)),atom('mdat',media)]);
  const changed=Buffer.from(movie);changed[20]=17;changed[132]=99;
  assert.equal(quicktimeIdentity(movie),quicktimeIdentity(changed));
  changed[changed.length-1]^=1;assert.notEqual(quicktimeIdentity(movie),quicktimeIdentity(changed));
  assert.equal(quicktimeIdentity(Buffer.from('not a movie')),null);
  console.log('PASS: ISO associated forks, bounds, extraction, resume, conflicts and QuickTime identity');
}
if(typeof process!=='undefined')run().catch(e=>{console.error(e);process.exitCode=1;});
