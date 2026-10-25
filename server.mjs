import { createServer } from 'node:http';
import { handler } from './build/handler.js';
import { applyIsolationHeaders } from './server/headers.js';

// adapter-node serves immutable worker files before SvelteKit's request hook.
const server = createServer((request, response) => {
	applyIsolationHeaders(response);
	handler(request, response);
});
const address = process.env.SOCKET_PATH
	? { path: process.env.SOCKET_PATH }
	: { host: process.env.HOST ?? '0.0.0.0', port: Number(process.env.PORT ?? 3000) };
server.listen(address);
for (const signal of ['SIGINT', 'SIGTERM']) {
	process.once(signal, () => {
		server.close(() => process.exit(0));
		setTimeout(() => server.closeAllConnections(), 30000).unref();
	});
}
