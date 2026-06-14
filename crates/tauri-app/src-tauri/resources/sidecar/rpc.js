export class JsonRpcServer {
    handlers = new Map();
    eventSink;
    constructor(eventSink) {
        this.eventSink = eventSink;
    }
    register(method, handler) {
        this.handlers.set(method, handler);
    }
    emit(event) {
        this.eventSink(event);
    }
    async handleLine(line) {
        const trimmed = line.trim();
        if (!trimmed)
            return;
        let request;
        try {
            request = JSON.parse(trimmed);
        }
        catch {
            return;
        }
        const response = { id: request.id ?? null };
        try {
            const handler = this.handlers.get(request.method);
            if (!handler) {
                response.error = { code: -32601, message: `Method not found: ${request.method}` };
            }
            else {
                response.result = await handler(request.params);
            }
        }
        catch (error) {
            response.error = {
                code: -32000,
                message: error instanceof Error ? error.message : String(error),
            };
        }
        process.stdout.write(`${JSON.stringify(response)}\n`);
    }
}
