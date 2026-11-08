import { db } from '../../../server/db';
import { games, categories } from '$lib/platform/catalog';
import { publicUserSelect } from '../../../server/auth';
export async function load({ url }: import('./$types').PageServerLoadEvent) {
	const ruleset = games.find((game) => game.id === url.searchParams.get('game'))?.id ?? 'chess';
	const category = categories.find((value) => value === url.searchParams.get('mode')) ?? 'blitz';
	const ratings = await db.rating.findMany({
		where: { ruleset, category, games: { gt: 0 } },
		include: { user: { select: publicUserSelect } },
		orderBy: [{ value: 'desc' }, { games: 'desc' }, { userId: 'asc' }],
		take: 100
	});
	return { ruleset, category, ratings };
}
