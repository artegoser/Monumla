import { error } from '@sveltejs/kit';
import { db } from '../../../../../server/db';
import { gameInclude } from '../../../../../server/games';
export async function GET({ params }: import('./$types').RequestEvent) {
	const game = await db.game.findUnique({
		where: { id: params.id },
		include: {
			...gameInclude,
			moves: {
				orderBy: { ply: 'asc' },
				where: { ply: { gt: 0 } },
				select: { notation: true, actor: true }
			}
		}
	});
	if (!game || game.ruleset !== 'chess') error(404, 'Chess game not found.');
	const escape = (value: string) => value.replaceAll('\\', '\\\\').replaceAll('"', '\\"');
	const tags = {
		Event: 'Monumla game',
		Site: 'Monumla',
		Date: game.createdAt.toISOString().slice(0, 10).replaceAll('-', '.'),
		White: game.firstUser?.username ?? `Computer level ${game.computerLevel}`,
		Black: game.secondUser?.username ?? `Computer level ${game.computerLevel}`,
		Result: game.result ?? '*',
		TimeControl: `${game.initialSeconds}+${game.increment}`,
		WhiteElo: String(game.firstRating),
		BlackElo: String(game.secondRating)
	};
	const moves = game.moves
		.map((move, i) => `${i % 2 === 0 ? `${Math.floor(i / 2) + 1}. ` : ''}${move.notation}`)
		.join(' ');
	return new Response(
		Object.entries(tags)
			.map(([key, value]) => `[${key} "${escape(value)}"]`)
			.join('\n') + `\n\n${moves} ${game.result ?? '*'}\n`,
		{
			headers: {
				'Content-Type': 'application/x-chess-pgn',
				'Content-Disposition': `attachment; filename="monumla-${game.id}.pgn"`
			}
		}
	);
}
