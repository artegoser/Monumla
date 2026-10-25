import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig, type Plugin } from 'vite';
import { applyIsolationHeaders } from './server/headers.js';

const isolation: Plugin = {
	name: 'monumla-worker-isolation',
	configureServer(server) {
		server.middlewares.use((_request, response, next) => {
			applyIsolationHeaders(response);
			next();
		});
	},
	configurePreviewServer(server) {
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
