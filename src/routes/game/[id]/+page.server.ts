import { error } from '@sveltejs/kit';
import { db } from '../../../../server/db';
import { gameInclude, gameSummary, sideFor } from '../../../../server/games';
export async function load({ params, locals }: import('./$types').PageServerLoadEvent) {
	const game = await db.game.findUnique({ where: { id: params.id }, include: gameInclude });
	if (!game) error(404, 'Game not found.');
	const side = sideFor(game, locals.user?.id);
	if (!side && game.ruleset === 'battleship' && game.status === 'active')
		error(403, 'Both fleets are private until the game ends.');
	const moves = await db.gameMove.findMany({
		where: { gameId: game.id },
		orderBy: { ply: 'asc' },
		select: { ply: true, actor: true, notation: true, viewOne: side !== 2, viewTwo: side === 2 }
	});
	return {
		game: gameSummary(game),
		side,
		moves: moves.map((move) => ({
			ply: move.ply,
			actor: move.actor,
			notation:
				game.ruleset === 'battleship' && game.status === 'active'
					? `Turn ${move.ply}`
					: move.notation,
			view: JSON.parse(
				(side === 2 ? move.viewTwo : move.viewOne)!
			) as import('$lib/wasm/runtime').GameView
		}))
	};
}
