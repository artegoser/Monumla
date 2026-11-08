import { error } from '@sveltejs/kit';
import { db } from '../../../../server/db';
import { publicUser, publicUserSelect } from '../../../../server/auth';
import { gameHistory } from '../../../../server/queries';
export async function load({ params, url, locals }: import('./$types').PageServerLoadEvent) {
	const user = await db.user.findUnique({
		where: { usernameKey: params.username.toLowerCase() },
		select: { ...publicUserSelect, ratings: { orderBy: [{ ruleset: 'asc' }, { category: 'asc' }] } }
	});
	if (!user) error(404, 'Player not found.');
	const page = Math.max(1, Math.min(100_000, Number(url.searchParams.get('page')) || 1));
	const history = await gameHistory(user.id, Math.floor(page));
	const relationship =
		locals.user && locals.user.id !== user.id
			? await db.friendship.findUnique({
					where: { pairKey: [locals.user.id, user.id].sort().join(':') }
				})
			: null;
	return {
		profile: publicUser(user),
		ratings: user.ratings,
		history,
		relationship: relationship
			? {
					id: relationship.id,
					status: relationship.status,
					incoming: relationship.recipientId === locals.user?.id
				}
			: null
	};
}
