import { after, before, test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { createServer } from 'node:http';
import { WebSocket } from 'ws';

const directory = mkdtempSync(join(tmpdir(), 'monumla-platform-'));
process.env.DATABASE_URL = `file:${join(directory, 'test.db')}`;
process.env.ORIGIN = 'http://127.0.0.1';
execFileSync('node_modules/.bin/prisma', ['migrate', 'deploy'], {
	env: process.env,
	stdio: 'pipe'
});
const { db } = await import('../../server/db');
const { hashPassword, verifyPassword, createSession, authenticate, tokenHash, profileFields } =
	await import('../../server/auth');
const { createEngine, encode, restoreEngine, wire } = await import('../../server/engine');
const { GameService } = await import('../../server/games');
const { attachRealtime } = await import('../../server/realtime');
const { games, categoryFor } = await import('../../src/lib/platform/catalog');
const options = { ruleset: 'tictactoe' as const, initialSeconds: 300, increment: 0, rated: true };
let count = 0;
async function user() {
	const name = `player_${++count}`;
	return db.user.create({
		data: {
			username: name,
			usernameKey: name,
			email: `${name}@example.com`,
			passwordHash: await hashPassword('a secure test password')
		}
	});
}
const server = createServer((_request, response) => response.end('ok'));
let realtime: ReturnType<typeof attachRealtime>;
let port: number;
const sockets = new Set<WebSocket>();
before(async () => {
	realtime = attachRealtime(server);
	await realtime.ready;
	await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
	port = (server.address() as import('node:net').AddressInfo).port;
});
after(async () => {
	for (const socket of sockets) socket.terminate();
	await realtime.close();
	await new Promise<void>((resolve) => server.close(() => resolve()));
	await db.$disconnect();
	rmSync(directory, { recursive: true, force: true });
});

async function client(userId?: string) {
	const token = userId ? await createSession(userId) : null;
	const socket = new WebSocket(`ws://127.0.0.1:${port}/ws`, {
		origin: process.env.ORIGIN,
		headers: token ? { Cookie: `monumla_session=${token}` } : {}
	});
	sockets.add(socket);
	const events: { type?: string; data?: unknown; id?: string; ok?: boolean; message?: string }[] =
		[];
	const requests = new Map<
		string,
		{ resolve: (data: unknown) => void; reject: (error: Error) => void }
	>();
	let id = 0;
	socket.on('message', (raw) => {
		const message = JSON.parse(raw.toString());
		events.push(message);
		if (!message.id) return;
		const pending = requests.get(message.id);
		if (pending) {
			requests.delete(message.id);
			if (message.ok) pending.resolve(message.data);
			else pending.reject(new Error(message.message));
		}
	});
	await new Promise<void>((resolve, reject) => {
		socket.once('open', resolve);
		socket.once('error', reject);
	});
	return {
		socket,
		events,
		async rpc<T = unknown>(type: string, data: Record<string, unknown> = {}) {
			const requestId = String(++id);
			return new Promise<T>((resolve, reject) => {
				const timer = setTimeout(() => {
					requests.delete(requestId);
					reject(new Error('WebSocket test request timed out'));
				}, 5000);
				requests.set(requestId, {
					resolve: (value) => {
						clearTimeout(timer);
						resolve(value as T);
					},
					reject: (error) => {
						clearTimeout(timer);
						reject(error);
					}
				});
				socket.send(JSON.stringify({ id: requestId, type, ...data }));
			});
		}
	};
}

async function until<T>(
	get: () => Promise<T>,
	predicate: (value: T) => boolean,
	timeout = 7000
): Promise<T> {
	const deadline = Date.now() + timeout;
	while (Date.now() < deadline) {
		const value = await get();
		if (predicate(value)) return value;
		await new Promise((resolve) => setTimeout(resolve, 25));
	}
	throw new Error('Condition was not reached before timeout');
}

test('password hashing, session revocation, country validation, and time categories', async () => {
	const hash = await hashPassword('a secure test password');
	assert.equal(await verifyPassword('a secure test password', hash), true);
	assert.equal(await verifyPassword('wrong password', hash), false);
	assert.notEqual(hash, await hashPassword('a secure test password'));
	const account = await user();
	const token = await createSession(account.id);
	const session = await authenticate(token);
	assert.equal(session?.id, account.id);
	assert.equal(session && 'passwordHash' in session, false);
	await db.session.delete({ where: { tokenHash: tokenHash(token) } });
	assert.equal(await authenticate(token), null);
	assert.throws(() => profileFields('XX', 'bio'));
	assert.throws(() => profileFields('GB', 'x'.repeat(501)));
	assert.deepEqual(profileFields('GB', ' hello '), { country: 'GB', bio: 'hello' });
	assert.deepEqual(
		[
			[60, 0],
			[120, 1],
			[180, 0],
			[300, 3],
			[600, 0],
			[900, 10],
			[1800, 0]
		].map(([s, i]) => categoryFor(s, i)),
		['bullet', 'bullet', 'blitz', 'blitz', 'rapid', 'rapid', 'classical']
	);
});

test('all six rulesets round-trip complete authoritative state', () => {
	for (const game of games) {
		const engine = createEngine(game.id);
		try {
			engine.setPerspective(1);
			const restored = restoreEngine(encode(engine.exportState()));
			try {
				restored.setPerspective(1);
				assert.deepEqual(wire(restored.view()), wire(engine.view()), game.id);
			} finally {
				restored.free();
			}
		} finally {
			engine.free();
		}
	}
});

test('Battleship persistence keeps both fleets while public views conceal the opponent', () => {
	const engine = createEngine('battleship');
	try {
		for (const side of [1, 2]) {
			engine.setPerspective(side);
			const arrange = engine
				.interaction()
				.choices.find((choice) => choice.option_key === 'arrange');
			assert.ok(arrange);
			engine.choose(arrange.id);
			const ready = engine.interaction().choices.find((choice) => choice.option_key === 'ready');
			assert.ok(ready);
			engine.choose(ready.id);
		}
		const saved = encode(engine.exportState());
		const restored = restoreEngine(saved);
		try {
			for (const side of [1, 2]) {
				restored.setPerspective(side);
				engine.setPerspective(side);
				assert.deepEqual(wire(restored.view()), wire(engine.view()));
				assert.ok(restored.view().entities.some((entity) => entity.owner === side));
				assert.equal(
					restored.view().entities.some((entity) => entity.owner !== side),
					false
				);
			}
		} finally {
			restored.free();
		}
	} finally {
		engine.free();
	}
});

test('rated results, history, replay, and Elo are committed once and only to the selected pool', async () => {
	const first = await user(),
		second = await user();
	const service = new GameService();
	try {
		const game = await service.create(options, first.id, second.id);
		for (const [x, y] of [
			[0, 0],
			[0, 1],
			[1, 0],
			[1, 1],
			[2, 0]
		]) {
			const record = await db.game.findUniqueOrThrow({ where: { id: game.id } });
			const userId = record.activeSide === 1 ? first.id : second.id;
			const state = await service.state(game.id, userId);
			const choice = state.interaction.choices.find(
				(candidate) => candidate.position?.x === x && candidate.position?.y === y
			);
			assert.ok(choice);
			await service.play(game.id, userId, choice.id, state.revision);
		}
		const result = await db.game.findUniqueOrThrow({
			where: { id: game.id },
			include: { moves: true }
		});
		assert.equal(result.status, 'finished');
		assert.equal(result.result, '1-0');
		assert.equal(result.firstDelta, 20);
		assert.equal(result.secondDelta, -20);
		assert.equal(result.moves.length, 6);
		const ratings = await db.rating.findMany({
			where: { userId: { in: [first.id, second.id] } },
			orderBy: { value: 'desc' }
		});
		assert.deepEqual(
			ratings.map((rating) => [rating.value, rating.games, rating.ruleset, rating.category]),
			[
				[1520, 1, 'tictactoe', 'blitz'],
				[1480, 1, 'tictactoe', 'blitz']
			]
		);
		await assert.rejects(service.adjudicate(game.id, second.id, 'resign'));
		assert.equal((await db.rating.findUniqueOrThrow({ where: { id: ratings[0].id } })).games, 1);
		await service.close();
		const restarted = new GameService();
		try {
			const state = await restarted.state(game.id, first.id);
			assert.equal(state.history.length, 5);
			assert.equal(state.result, '1-0');
		} finally {
			await restarted.close();
		}
	} finally {
		await service.close();
	}
});

test('ownership, revision checks, stale choices, and resignation are authoritative', async () => {
	const first = await user(),
		second = await user(),
		stranger = await user();
	const service = new GameService();
	try {
		const game = await service.create({ ...options, rated: false }, first.id, second.id);
		const state = await service.state(game.id, first.id);
		const choice = state.interaction.choices[0].id;
		await assert.rejects(
			service.play(game.id, stranger.id, choice, state.revision),
			/not your turn/
		);
		await assert.rejects(service.play(game.id, second.id, choice, state.revision), /not your turn/);
		await assert.rejects(
			service.play(game.id, first.id, choice, state.revision + 1),
			/position changed/
		);
		assert.equal((await service.state(game.id, stranger.id)).interaction.choices.length, 0);
		await service.play(game.id, first.id, choice, state.revision);
		await assert.rejects(service.play(game.id, first.id, choice, state.revision), /not your turn/);
		await service.adjudicate(game.id, second.id, 'resign');
		assert.equal((await service.state(game.id, first.id)).result, '1-0');
		assert.equal(await db.rating.count({ where: { userId: { in: [first.id, second.id] } } }), 0);
	} finally {
		await service.close();
	}
});

test('clocks survive restart and timeout adjudication is idempotent', async () => {
	const first = await user(),
		second = await user();
	const service = new GameService();
	const game = await service.create({ ...options, initialSeconds: 60 }, first.id, second.id);
	await service.close();
	await db.game.update({
		where: { id: game.id },
		data: { turnStartedAt: new Date(Date.now() - 61_000) }
	});
	const restarted = new GameService();
	try {
		await restarted.tick();
		await restarted.tick();
		const result = await db.game.findUniqueOrThrow({ where: { id: game.id } });
		assert.equal(result.result, '0-1');
		assert.equal(result.termination, 'timeout');
		assert.equal((await db.rating.findFirstOrThrow({ where: { userId: second.id } })).games, 1);
	} finally {
		await restarted.close();
	}
});

test('WebSocket matchmaking pairs compatible players and clears their queues atomically', async () => {
	const first = await user(),
		second = await user();
	const a = await client(first.id),
		b = await client(second.id);
	try {
		await a.rpc('queue.join', options);
		await b.rpc('queue.join', options);
		const state = await a.rpc<import('../../src/lib/platform/types').Personal>('personal');
		assert.ok(state.activeGame);
		assert.equal(state.queue, null);
		const game = state.activeGame;
		const other = await b.rpc<import('../../src/lib/platform/types').Personal>('personal');
		assert.equal(other.activeGame?.id, game.id);
		const aState = await a.rpc<import('../../src/lib/platform/types').LiveGame>('game.watch', {
			gameId: game.id
		});
		const bState = await b.rpc<import('../../src/lib/platform/types').LiveGame>('game.watch', {
			gameId: game.id
		});
		assert.notEqual(aState.side, bState.side);
		const moving = aState.activeSide === aState.side ? a : b;
		const move = aState.activeSide === aState.side ? aState : bState;
		await moving.rpc('game.choose', {
			gameId: game.id,
			choice: move.interaction.choices[0].id,
			revision: move.revision
		});
		await until(
			async () =>
				a.events.some((event) => event.type === 'game') &&
				b.events.some((event) => event.type === 'game'),
			(received) => received
		);
		await a.rpc('game.resign', { gameId: game.id });
	} finally {
		a.socket.terminate();
		b.socket.terminate();
	}
});

test('friend requests and challenges enforce recipient authorization and notify both players', async () => {
	const first = await user(),
		second = await user(),
		third = await user();
	const a = await client(first.id),
		b = await client(second.id),
		c = await client(third.id);
	try {
		await a.rpc('friend.request', { userId: second.id });
		const state = await b.rpc<import('../../src/lib/platform/types').Personal>('personal');
		const friend = state.friends.find((item) => item.user.id === first.id)!;
		assert.equal(friend.incoming, true);
		await assert.rejects(c.rpc('friend.accept', { friendshipId: friend.id }));
		await b.rpc('friend.accept', { friendshipId: friend.id });
		assert.equal(
			(await db.friendship.findUniqueOrThrow({ where: { id: friend.id } })).status,
			'accepted'
		);
		const challenge = await a.rpc<{ challengeId: string }>('challenge.create', {
			...options,
			userId: second.id
		});
		await assert.rejects(c.rpc('challenge.accept', challenge));
		const match = await b.rpc<{ gameId: string }>('challenge.accept', challenge);
		assert.equal(
			(await db.challenge.findUniqueOrThrow({ where: { id: challenge.challengeId } })).status,
			'accepted'
		);
		await assert.rejects(b.rpc('challenge.accept', challenge));
		await a.rpc('game.resign', match);
		await a.rpc('friend.remove', { friendshipId: friend.id });
		assert.equal(await db.friendship.findUnique({ where: { id: friend.id } }), null);
	} finally {
		a.socket.terminate();
		b.socket.terminate();
		c.socket.terminate();
	}
});

test('unauthenticated sockets cannot mutate games and foreign origins cannot connect', async () => {
	const guest = await client();
	try {
		await assert.rejects(guest.rpc('queue.join', options), /Sign in/);
	} finally {
		guest.socket.terminate();
	}
	await new Promise<void>((resolve, reject) => {
		const socket = new WebSocket(`ws://127.0.0.1:${port}/ws`, {
			origin: 'https://attacker.example'
		});
		socket.once('open', () => {
			socket.terminate();
			reject(new Error('Foreign origin was accepted'));
		});
		socket.once('error', () => resolve());
	});
});

test('computer moves run in a worker and games remain casual', async () => {
	await realtime.close();
	const human = await user();
	const service = new GameService();
	try {
		const game = await service.create({ ...options, rated: false }, null, human.id, 1);
		await service.tick();
		const state = await until(
			() => service.state(game.id, human.id),
			(state) => state.revision > 0
		);
		assert.equal(state.history.length, 1);
		assert.equal(state.activeSide, 2);
		assert.equal(state.rated, false);
		await service.adjudicate(game.id, human.id, 'resign');
	} finally {
		await service.close();
	}
});
