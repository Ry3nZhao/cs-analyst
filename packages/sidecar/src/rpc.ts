import type { JsonRpcRequest, JsonRpcResponse, SidecarEvent } from '@cs-demo-analyst/shared-types';

type Handler = (params: unknown) => Promise<unknown>;

export class JsonRpcServer {
  private handlers = new Map<string, Handler>();
  private eventSink: (event: SidecarEvent) => void;

  constructor(eventSink: (event: SidecarEvent) => void) {
    this.eventSink = eventSink;
  }

  register(method: string, handler: Handler) {
    this.handlers.set(method, handler);
  }

  emit(event: SidecarEvent) {
    this.eventSink(event);
  }

  async handleLine(line: string) {
    const trimmed = line.trim();
    if (!trimmed) return;
    let request: JsonRpcRequest;
    try {
      request = JSON.parse(trimmed) as JsonRpcRequest;
    } catch {
      return;
    }
    const response: JsonRpcResponse = { id: request.id ?? null };
    try {
      const handler = this.handlers.get(request.method);
      if (!handler) {
        response.error = { code: -32601, message: `Method not found: ${request.method}` };
      } else {
        response.result = await handler(request.params);
      }
    } catch (error) {
      response.error = {
        code: -32000,
        message: error instanceof Error ? error.message : String(error),
      };
    }
    process.stdout.write(`${JSON.stringify(response)}\n`);
  }
}
