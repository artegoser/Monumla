import type { Handle } from '@sveltejs/kit';
import { isolationHeaders } from '../server/headers.js';
import { authenticate, SESSION_COOKIE } from '../server/auth';

export const handle: Handle = async ({ event, resolve }) => {
	event.locals.user = await authenticate(event.cookies.get(SESSION_COOKIE));
	const response = await resolve(event);
	response.headers.set('X-Content-Type-Options', 'nosniff');
	response.headers.set('Referrer-Policy', 'strict-origin-when-cross-origin');
	if (event.locals.user) response.headers.set('Cache-Control', 'private, no-store');
	for (const [name, value] of Object.entries(isolationHeaders)) response.headers.set(name, value);
	return response;
};
