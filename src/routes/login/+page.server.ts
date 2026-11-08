import { fail, redirect } from '@sveltejs/kit';
import { db } from '../../../server/db';
import {
	createSession,
	verifyPassword,
	rateLimit,
	SESSION_COOKIE,
	SESSION_SECONDS,
	userMessage
} from '../../../server/auth';

export function load({ locals }: import('./$types').PageServerLoadEvent) {
	if (locals.user) redirect(303, '/');
}
export const actions = {
	default: async ({ request, cookies, url, getClientAddress }: import('./$types').RequestEvent) => {
		const data = await request.formData();
		const identity = String(data.get('identity') ?? '')
			.trim()
			.toLowerCase();
		const password = String(data.get('password') ?? '');
		let token: string;
		try {
			rateLimit(`login:${getClientAddress()}`, 12);
			if (!identity || identity.length > 254 || password.length > 128)
				throw new Error('Check your username and password.');
			const user = await db.user.findFirst({
				where: { OR: [{ usernameKey: identity }, { email: identity }] }
			});
			const valid = await verifyPassword(
				password,
				user?.passwordHash ?? `${'0'.repeat(32)}:${'0'.repeat(128)}`
			);
			if (!user || !valid) throw new Error('Check your username and password.');
			token = await createSession(user.id);
		} catch (error) {
			return fail(400, {
				message: userMessage(error, 'Sign in failed.'),
				identity
			});
		}
		cookies.set(SESSION_COOKIE, token, {
			path: '/',
			httpOnly: true,
			sameSite: 'lax',
			secure: url.protocol === 'https:',
			maxAge: SESSION_SECONDS
		});
		redirect(303, '/');
	}
};
