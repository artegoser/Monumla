import { createServer } from 'node:http';
import { handler } from './build/handler.js';
import { applyIsolationHeaders } from './server/headers.js';
import { attachRealtime } from './server/realtime.ts';

// adapter-node serves immutable worker files before SvelteKit's request hook.
const server = createServer((request, response) => {
	applyIsolationHeaders(response);
	handler(request, response);
});
const address = process.env.SOCKET_PATH
	? { path: process.env.SOCKET_PATH }
	: { host: process.env.HOST ?? '0.0.0.0', port: Number(process.env.PORT ?? 3000) };
const realtime = attachRealtime(server);
await realtime.ready;
server.listen(address);
for (const signal of ['SIGINT', 'SIGTERM']) {
	process.once(signal, async () => {
		await realtime.close();
		server.close(() => process.exit(0));
		setTimeout(() => server.closeAllConnections(), 30000).unref();
	});
}
