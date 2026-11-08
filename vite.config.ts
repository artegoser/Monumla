import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig, type Plugin } from 'vite';
import { applyIsolationHeaders } from './server/headers.js';
import { attachRealtime } from './server/realtime';

const isolation: Plugin = {
	name: 'monumla-worker-isolation',
	configureServer(server) {
		if (server.httpServer) {
			const realtime = attachRealtime(server.httpServer as import('node:http').Server);
			server.httpServer.once('close', () => {
				void realtime.close();
			});
		}
		server.middlewares.use((_request, response, next) => {
			applyIsolationHeaders(response);
			next();
		});
	},
	configurePreviewServer(server) {
		const realtime = attachRealtime(server.httpServer as import('node:http').Server);
		server.httpServer.once('close', () => {
			void realtime.close();
		});
		server.middlewares.use((_request, response, next) => {
			applyIsolationHeaders(response);
			next();
		});
	}
};

export default defineConfig({
	plugins: [isolation, sveltekit()],
	worker: { format: 'es' }
});
