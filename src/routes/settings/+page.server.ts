import { fail, redirect } from '@sveltejs/kit';
import { db } from '../../../server/db';
import { profileFields, userMessage } from '../../../server/auth';
export function load({ locals }: import('./$types').PageServerLoadEvent) {
	if (!locals.user) redirect(303, '/login');
	return { profile: locals.user };
}
export const actions = {
	default: async ({ locals, request }: import('./$types').RequestEvent) => {
		if (!locals.user) redirect(303, '/login');
		const form = await request.formData();
		const country = String(form.get('country') ?? '');
		const bio = String(form.get('bio') ?? '');
		try {
			await db.user.update({ where: { id: locals.user.id }, data: profileFields(country, bio) });
		} catch (error) {
			return fail(400, {
				message: userMessage(error, 'Your profile could not be saved.'),
				country,
				bio,
				saved: false
			});
		}
		return { saved: true, message: 'Your profile has been saved.', country, bio };
	}
};
