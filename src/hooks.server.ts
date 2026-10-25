import type { Handle } from '@sveltejs/kit';
import { isolationHeaders } from '../server/headers.js';

export const handle: Handle = async ({ event, resolve }) => {
	const response = await resolve(event);
	for (const [name, value] of Object.entries(isolationHeaders)) response.headers.set(name, value);
	return response;
};
