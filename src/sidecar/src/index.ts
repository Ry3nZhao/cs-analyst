#!/usr/bin/env node
import readline from 'node:readline';
import { PROTOCOL_VERSION } from '@cs-analyst/downloader-protocol';
import { JsonRpcServer } from './rpc.js';
import { MODULE_VERSION, registerHandlers, sidecarLog } from './handlers.js';

const server = new JsonRpcServer();
registerHandlers(server);

// adapter 里 EventEmitter 抛出的异常（例如 steam-user 的 error 事件）不能拖垮其他平台：记录后继续运行。
// 进程若因此卡死，主程序的心跳会发现并重启。
process.on('uncaughtException', (error) => {
  sidecarLog('uncaughtException', { message: error.message, stack: error.stack });
});
process.on('unhandledRejection', (reason) => {
  sidecarLog('unhandledRejection', { message: reason instanceof Error ? reason.message : String(reason) });
});

const rl = readline.createInterface({ input: process.stdin });
rl.on('line', (line) => {
  void server.handleLine(line);
});
// 主程序关闭 stdin（退出或主动停止）→ 跟着退出，避免残留进程
rl.on('close', () => process.exit(0));

server.emit({ event: 'sidecar.ready', protocolVersion: PROTOCOL_VERSION, version: MODULE_VERSION });
