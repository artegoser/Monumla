import { personal } from '../../server/queries';

export async function load({ locals }: import('./$types').LayoutServerLoadEvent) {
	return { user: locals.user, personal: locals.user ? await personal(locals.user.id) : null };
}
