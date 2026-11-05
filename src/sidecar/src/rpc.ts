import {
  RpcErrorCode,
  type JsonRpcRequest,
  type JsonRpcResponse,
  type RpcErrorData,
  type SidecarEvent,
} from '@cs-analyst/downloader-protocol';

type Handler = (params: unknown) => Promise<unknown>;
type Writer = (line: string) => void;

/** 带错误码的 RPC 错误；其他异常一律按 `Internal` 返回。 */
export class RpcError extends Error {
  constructor(
    readonly code: number,
    message: string,
    readonly data?: RpcErrorData,
  ) {
    super(message);
  }
}

export class JsonRpcServer {
  private handlers = new Map<string, Handler>();
  private inflightCount = 0;
  private readonly write: Writer;

  constructor(write: Writer = (line) => process.stdout.write(`${line}\n`)) {
    this.write = write;
  }

  register(method: string, handler: Handler) {
    this.handlers.set(method, handler);
  }

  methods(): string[] {
    return [...this.handlers.keys()];
  }

  get inflight(): number {
    return this.inflightCount;
  }

  emit(event: SidecarEvent) {
    this.write(JSON.stringify(event));
  }

  async handleLine(line: string) {
    const trimmed = line.trim();
    if (!trimmed) return;
    let request: JsonRpcRequest;
    try {
      request = JSON.parse(trimmed) as JsonRpcRequest;
    } catch {
      this.respond({ id: null, error: { code: RpcErrorCode.ParseError, message: 'Parse error' } });
      return;
    }
    if (!request || typeof request !== 'object' || typeof request.method !== 'string') {
      this.respond({
        id: (request as { id?: string | number } | null)?.id ?? null,
        error: { code: RpcErrorCode.InvalidRequest, message: 'Invalid request' },
      });
      return;
    }

    const response: JsonRpcResponse = { id: request.id ?? null };
    const handler = this.handlers.get(request.method);
    if (!handler) {
      response.error = { code: RpcErrorCode.MethodNotFound, message: `Method not found: ${request.method}` };
      this.respond(response);
      return;
    }
    this.inflightCount += 1;
    try {
      response.result = (await handler(request.params ?? {})) ?? null;
    } catch (error) {
      if (error instanceof RpcError) {
        response.error = { code: error.code, message: error.message, ...(error.data ? { data: error.data } : {}) };
      } else {
        response.error = {
          code: RpcErrorCode.Internal,
          message: error instanceof Error ? error.message : String(error),
        };
      }
    } finally {
      this.inflightCount -= 1;
    }
    this.respond(response);
  }

  private respond(response: JsonRpcResponse) {
    this.write(JSON.stringify(response));
  }
}
