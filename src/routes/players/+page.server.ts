import { searchUsers } from '../../../server/queries';
export async function load({ url }: import('./$types').PageServerLoadEvent) {
	const query = (url.searchParams.get('q') ?? '').slice(0, 30);
	return { query, players: await searchUsers(query) };
}
