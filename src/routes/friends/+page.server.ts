import { redirect } from '@sveltejs/kit';
export function load({ locals }: import('./$types').PageServerLoadEvent) {
	if (!locals.user) redirect(303, '/login');
}
