import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { AitProtocolClient } from '../../../tests/ai-conformance/src/protocol-client.mjs';
const port=25464;
const client=new AitProtocolClient({baseUrl:`http://127.0.0.1:${port}`});
const channel=`private-c1-${Date.now()}`;
async function wait(predicate,label) {
  const deadline=Date.now()+5000;
  while(Date.now()<deadline) {const value=predicate();if(value)return value;await new Promise(r=>setTimeout(r,5));}
  throw new Error(`timed out: ${label}`);
}
async function connect(protocol) {
  const frames=[];
  const socket=new WebSocket(`ws://127.0.0.1:${port}/app/app-key?protocol=${protocol}&client=c1&version=0`);
  socket.addEventListener('message',e=>frames.push(JSON.parse(String(e.data))));
  const established=await wait(()=>frames.find(f=>f.event.endsWith(':connection_established')),'connection');
  const data=typeof established.data==='string'?JSON.parse(established.data):established.data;
  const signature=crypto.createHmac('sha256','app-secret').update(`${data.socket_id}:${channel}`).digest('hex');
  socket.send(JSON.stringify({event:'pusher:subscribe',data:{channel,auth:`app-key:${signature}`}}));
  await wait(()=>frames.find(f=>f.event===(protocol===2?'sockudo_internal:subscription_succeeded':'pusher_internal:subscription_succeeded')),'authenticated subscription');
  return {socket,frames,socketId:data.socket_id};
}
const v1=await connect(7);
const v2=await connect(2);
try {
  const create=await client.publish({name:'c1-event',channel,data:'seed',messageId:`c1-${Date.now()}`});
  const messageSerial=create.channels[channel].message_serial;
  assert.ok(messageSerial);
  const v1create=await wait(()=>v1.frames.find(f=>f.event==='c1-event'),'V1 create');
  assert.equal(typeof v1create.data,'string');
  for(const field of ['message_serial','history_serial','delivery_serial','version','action','extras','stream_id','serial']) assert.equal(Object.hasOwn(v1create,field),false,`V1 stripped ${field}`);
  const requests=Array.from({length:8},(_,i)=>({channel,messageSerial,data:`[${i}]`,opId:`c1-op-${i}`}));
  const receipts=await Promise.all(requests.map(request=>client.append(request)));
  assert.deepEqual(receipts.map(r=>r.delivery_serial).sort((a,b)=>a-b),[2,3,4,5,6,7,8,9]);
  const latest=await client.getMessage({channel,messageSerial});
  for(let i=0;i<8;i++) assert.equal(latest.item.data.split(`[${i}]`).length-1,1);
  assert.equal(latest.item.delivery_serial,9);
  const duplicate=await client.append(requests[0]);
  assert.equal(duplicate.status,'duplicate');
  assert.equal(duplicate.delivery_serial,receipts[0].delivery_serial);
  await assert.rejects(client.append({...requests[0],data:'changed'}),/HTTP 400:.*idempotency key was already used with a different payload/);
  await wait(()=>v2.frames.filter(f=>f.event==='sockudo:message.append').length===8,'all original append deliveries');
  assert.deepEqual(v2.frames.filter(f=>f.event==='sockudo:message.append').map(f=>f.serial),[2,3,4,5,6,7,8,9]);
  // An unsigned-in socket cannot turn a claimed client ID into owner proof.
  const path=`/channels/${encodeURIComponent(channel)}/messages/${encodeURIComponent(messageSerial)}`;
  await assert.rejects(client.signedJson('POST',`${path}/update`,{data:'spoofed',socket_id:v2.socketId,client_id:'actor'}),/HTTP 401:.*Mutation actor socket is not signed in with an identified client/);
  assert.equal((await client.getMessage({channel,messageSerial})).item.data,latest.item.data);
  const deleted=await client.signedJson('POST',`${path}/delete`,{clear_fields:['data']});
  assert.equal(deleted.delivery_serial,10);
  assert.equal((await client.getMessage({channel,messageSerial})).item.action,'delete');
  const versions=await client.signedJson('GET',`${path}/versions`);
  assert.equal(versions.items.length,10);
  for(const frame of v1.frames) for(const field of ['message_serial','history_serial','delivery_serial','version','action','extras','stream_id','serial']) assert.equal(Object.hasOwn(frame,field),false);
  console.log(JSON.stringify({ok:true,concurrentAppends:8,originalVersions:10,duplicateReceipt:true,conflictingReceiptRejected:true,actorSpoofRejected:true,v1FieldsStripped:true,v2DeliverySerialsContiguous:true}));
} finally {v1.socket.close();v2.socket.close();}
