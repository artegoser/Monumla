import { db } from './db';
import { gameInclude, gameSummary } from './games';
import { publicUser, publicUserSelect } from './auth';

export async function lobby() {
	const [players, played, playing, waiting, recent] = await Promise.all([
		db.user.count(),
		db.game.count({ where: { status: 'finished' } }),
		db.game.count({ where: { status: 'active' } }),
		db.queueEntry.groupBy({ by: ['ruleset'], _count: true }),
		db.game.findMany({
			where: { status: 'active', ruleset: { not: 'battleship' } },
			include: gameInclude,
			orderBy: { createdAt: 'desc' },
			take: 8
		})
	]);
	return {
		players,
		played,
		playing,
		waiting: Object.fromEntries(waiting.map((item) => [item.ruleset, item._count])),
		recent: recent.map(gameSummary)
	};
}

export async function personal(userId: string) {
	const [relationships, challenges, queue, activeGame] = await Promise.all([
		db.friendship.findMany({
			where: { OR: [{ senderId: userId }, { recipientId: userId }] },
			include: { sender: { select: publicUserSelect }, recipient: { select: publicUserSelect } },
			orderBy: { createdAt: 'desc' }
		}),
		db.challenge.findMany({
			where: {
				OR: [{ senderId: userId }, { recipientId: userId }],
				status: 'pending',
				expiresAt: { gt: new Date() }
			},
			include: { sender: { select: publicUserSelect }, recipient: { select: publicUserSelect } },
			orderBy: { createdAt: 'desc' },
			take: 30
		}),
		db.queueEntry.findUnique({ where: { userId } }),
		db.game.findFirst({
			where: { status: 'active', OR: [{ firstUserId: userId }, { secondUserId: userId }] },
			include: gameInclude
		})
	]);
	return {
		friends: relationships.map((item) => ({
			id: item.id,
			status: item.status,
			incoming: item.recipientId === userId,
			user: publicUser(item.senderId === userId ? item.recipient : item.sender)
		})),
		challenges: challenges.map((item) => ({
			id: item.id,
			incoming: item.recipientId === userId,
			user: publicUser(item.senderId === userId ? item.recipient : item.sender),
			ruleset: item.ruleset,
			initialSeconds: item.initialSeconds,
			increment: item.increment,
			rated: item.rated,
			expiresAt: item.expiresAt.toISOString()
		})),
		queue: queue
			? {
					ruleset: queue.ruleset,
					initialSeconds: queue.initialSeconds,
					increment: queue.increment,
					rated: queue.rated,
					joinedAt: queue.joinedAt.toISOString()
				}
			: null,
		activeGame: activeGame ? gameSummary(activeGame) : null
	};
}

export async function searchUsers(query: string) {
	return db.user.findMany({
		where: { usernameKey: { contains: query.trim().toLowerCase().slice(0, 30) } },
		select: { ...publicUserSelect, ratings: { orderBy: { value: 'desc' }, take: 3 } },
		orderBy: { usernameKey: 'asc' },
		take: 25
	});
}

export async function gameHistory(userId?: string, page = 1) {
	const where = userId
		? { OR: [{ firstUserId: userId }, { secondUserId: userId }] }
		: { status: 'finished' };
	const [total, rows] = await Promise.all([
		db.game.count({ where }),
		db.game.findMany({
			where,
			include: gameInclude,
			orderBy: { createdAt: 'desc' },
			skip: (page - 1) * 20,
			take: 20
		})
	]);
	return { total, games: rows.map(gameSummary), page };
}
