import { fail, redirect } from '@sveltejs/kit';
import { db } from '../../../server/db';
import {
	createSession,
	hashPassword,
	profileFields,
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
		const username = String(data.get('username') ?? '').trim();
		const email = String(data.get('email') ?? '')
			.trim()
			.toLowerCase();
		const password = String(data.get('password') ?? '');
		const country = String(data.get('country') ?? '');
		let token: string;
		try {
			rateLimit(`register:${getClientAddress()}`, 5);
			if (!/^[a-zA-Z0-9_-]{3,24}$/.test(username))
				throw new Error('Use 3–24 letters, numbers, underscores, or hyphens for your username.');
			if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email) || email.length > 254)
				throw new Error('Enter a valid email address.');
			if (password.length < 10 || password.length > 128)
				throw new Error('Choose a password between 10 and 128 characters.');
			const profile = profileFields(country, '');
			const user = await db.user.create({
				data: {
					username,
					usernameKey: username.toLowerCase(),
					email,
					passwordHash: await hashPassword(password),
					...profile
				}
			});
			token = await createSession(user.id);
		} catch (error) {
			const message =
				error && typeof error === 'object' && 'code' in error && error.code === 'P2002'
					? 'That username or email is already registered.'
					: userMessage(error, 'Registration failed. Please try again.');
			return fail(400, { message, username, email, country });
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
