// C1-only authenticated adapter for the audit's local raw-wire fixture.
// The upstream fixture subscribes to private channels without supplying auth.
import crypto from 'node:crypto';
import { AitProtocolClient } from '../../../tests/ai-conformance/src/protocol-client.mjs';
const connect = AitProtocolClient.prototype.connect;
AitProtocolClient.prototype.connect = async function () {
  const session = await connect.call(this);
  const subscribe = session.subscribe.bind(session);
  session.subscribe = (channel, extra = {}) => {
    const socketId = session.transcript.find(f => f.event === 'sockudo:connection_established').data.socket_id;
    const signature = crypto.createHmac('sha256', this.secret).update(`${socketId}:${channel}`).digest('hex');
    return subscribe(channel, { auth: `${this.key}:${signature}`, ...extra });
  };
  return session;
};
await import('../../../tests/ai-conformance/src/run.mjs');
