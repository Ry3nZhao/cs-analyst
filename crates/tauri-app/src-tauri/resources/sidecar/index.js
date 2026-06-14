#!/usr/bin/env node
import readline from 'node:readline';
import { JsonRpcServer } from './rpc.js';
import { registerHandlers } from './handlers.js';
function emitEvent(event) {
    process.stdout.write(`${JSON.stringify(event)}\n`);
}
const server = new JsonRpcServer(emitEvent);
registerHandlers(server);
const rl = readline.createInterface({ input: process.stdin });
rl.on('line', (line) => {
    void server.handleLine(line);
});
emitEvent({ event: 'sidecar.ready' });
