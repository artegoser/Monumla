import { redirect } from '@sveltejs/kit';
import { db } from '../../../server/db';
import { SESSION_COOKIE, tokenHash } from '../../../server/auth';
export async function POST({ cookies, request, url }: import('./$types').RequestEvent) {
	if (request.headers.get('origin') !== url.origin)
		return new Response('Invalid origin', { status: 403 });
	const token = cookies.get(SESSION_COOKIE);
	if (token) await db.session.deleteMany({ where: { tokenHash: tokenHash(token) } });
	cookies.delete(SESSION_COOKIE, { path: '/' });
	redirect(303, '/');
}
