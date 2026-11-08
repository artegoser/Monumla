import type { Server, IncomingMessage } from 'node:http';
import type { Duplex } from 'node:stream';
import { WebSocket, WebSocketServer } from 'ws';
import { authenticate, readSessionCookie, rateLimit, userMessage } from './auth';
import { db, initializeDatabase } from './db';
import { GameService, matchOptions, gameInclude, gameSummary, type MatchOptions } from './games';
import { lobby, personal, searchUsers } from './queries';
import { categoryFor, type PublicUser } from '../src/lib/platform/catalog';

interface Client {
	socket: WebSocket;
	user: PublicUser | null;
	token?: string;
	watching: string | null;
	alive: boolean;
}
interface Message extends Record<string, unknown> {
	id: string;
	type: string;
}
export function attachRealtime(server: Server) {
	const wss = new WebSocketServer({ noServer: true, maxPayload: 16_384, perMessageDeflate: false });
	const clients = new Set<Client>();
	const service = new GameService();
	let tail: Promise<unknown> = Promise.resolve();
	let stopped = false;
	let ticking = false;
	function serialize<T>(task: () => Promise<T>): Promise<T> {
		const next = tail.then(task);
		tail = next.catch(() => {});
		return next;
	}
	service.serialize = serialize;
	function send(client: Client, data: unknown) {
		if (client.socket.readyState !== WebSocket.OPEN) return;
		if (client.socket.bufferedAmount > 4_000_000) {
			client.socket.close(1013, 'Reconnect to refresh your board.');
			return;
		}
		client.socket.send(JSON.stringify(data));
	}
	function online() {
		return [
			...new Set([...clients].filter((client) => client.user).map((client) => client.user!.id))
		];
	}
	async function broadcastLobby() {
		const data = { ...(await lobby()), online: online().length };
		for (const client of clients) send(client, { type: 'lobby', data });
	}
	async function notify(userIds: string[]) {
		for (const userId of new Set(userIds)) {
			const targets = [...clients].filter((client) => client.user?.id === userId);
			if (!targets.length) continue;
			const data = { ...(await personal(userId)), online: online() };
			for (const client of targets) send(client, { type: 'personal', data });
		}
	}
	service.onChange = async (id) => {
		for (const client of clients)
			if (client.watching === id) {
				try {
					send(client, { type: 'game', data: await service.state(id, client.user?.id) });
				} catch (error) {
					send(client, {
						type: 'notice',
						message: userMessage(error)
					});
				}
			}
		const game = await db.game.findUniqueOrThrow({ where: { id } });
		await notify(
			[game.firstUserId, game.secondUserId].filter((userId): userId is string => !!userId)
		);
		if (game.status === 'finished') {
			for (const client of clients) send(client, { type: 'ratings.changed' });
			await broadcastLobby();
		}
	};
	async function created(id: string) {
		const game = await db.game.findUniqueOrThrow({ where: { id }, include: gameInclude });
		for (const client of clients)
			if (client.user && [game.firstUserId, game.secondUserId].includes(client.user.id))
				send(client, { type: 'match', data: gameSummary(game) });
		await notify(
			[game.firstUserId, game.secondUserId].filter((userId): userId is string => !!userId)
		);
		await broadcastLobby();
	}
	async function tryMatch() {
		const queue = await db.queueEntry.findMany({ orderBy: { joinedAt: 'asc' }, take: 200 });
		const available = new Set(online());
		for (const entry of queue) {
			if (!available.has(entry.userId)) continue;
			const opponent = queue.find(
				(other) =>
					other.userId !== entry.userId &&
					available.has(other.userId) &&
					other.ruleset === entry.ruleset &&
					other.initialSeconds === entry.initialSeconds &&
					other.increment === entry.increment &&
					other.rated === entry.rated &&
					(!entry.rated ||
						Math.abs(other.rating - entry.rating) <=
							250 +
								Math.max(
									Date.now() - entry.joinedAt.getTime(),
									Date.now() - other.joinedAt.getTime()
								) /
									200)
			);
			if (!opponent) continue;
			const first = Math.random() < 0.5 ? entry.userId : opponent.userId;
			try {
				const game = await service.create(
					entry as MatchOptions,
					first,
					first === entry.userId ? opponent.userId : entry.userId
				);
				available.delete(entry.userId);
				available.delete(opponent.userId);
				await created(game.id);
			} catch (error) {
				available.delete(entry.userId);
				available.delete(opponent.userId);
				await db.queueEntry.deleteMany({
					where: { userId: { in: [entry.userId, opponent.userId] } }
				});
				await notify([entry.userId, opponent.userId]);
				console.error('Matchmaking failed:', error);
			}
		}
	}
	async function dispatch(client: Client, message: Message): Promise<unknown> {
		const { type } = message;
		if (client.token) client.user = await authenticate(client.token);
		if (type === 'lobby') return { ...(await lobby()), online: online().length };
		if (type === 'users.search') return searchUsers(String(message.query ?? ''));
		if (type === 'game.watch') {
			const id = String(message.gameId ?? '');
			if (id.length > 64) throw new Error('Invalid game.');
			const state = await service.state(id, client.user?.id);
			client.watching = id;
			return state;
		}
		if (type === 'game.unwatch') {
			client.watching = null;
			return null;
		}
		const user = await authenticate(client.token);
		if (!user) {
			client.user = null;
			throw new Error('Sign in to play and connect with other players.');
		}
		client.user = user;
		rateLimit(`ws:${user.id}`, 180);
		const userId = user.id;
		if (type === 'personal') return { ...(await personal(userId)), online: online() };
		if (type === 'queue.leave') {
			await db.queueEntry.deleteMany({ where: { userId } });
			await notify([userId]);
			await broadcastLobby();
			return null;
		}
		if (type === 'queue.join') {
			const options = matchOptions(message);
			const existing = await db.game.findFirst({
				where: { status: 'active', OR: [{ firstUserId: userId }, { secondUserId: userId }] }
			});
			if (existing)
				throw new Error('You have an active game. Return to it before joining another queue.');
			const category = categoryFor(options.initialSeconds, options.increment);
			const rating = await db.rating.findUnique({
				where: { userId_ruleset_category: { userId, ruleset: options.ruleset, category } }
			});
			await db.queueEntry.upsert({
				where: { userId },
				create: { userId, ...options, category, rating: rating?.value ?? 1500 },
				update: { ...options, category, rating: rating?.value ?? 1500, joinedAt: new Date() }
			});
			await tryMatch();
			await notify([userId]);
			await broadcastLobby();
			return null;
		}
		if (type === 'computer.create') {
			const options = matchOptions(message);
			const level = Number(message.level);
			const side = message.side === 2 ? 2 : 1;
			const game = await service.create(
				{ ...options, rated: false },
				side === 1 ? userId : null,
				side === 2 ? userId : null,
				level
			);
			await created(game.id);
			return { gameId: game.id };
		}
		if (type === 'game.choose' || type === 'game.cancel') {
			await service.play(
				String(message.gameId),
				userId,
				type === 'game.cancel' ? null : String(message.choice),
				Number(message.revision)
			);
			return null;
		}
		if (['game.resign', 'game.draw', 'game.draw.accept', 'game.draw.decline'].includes(type)) {
			await service.adjudicate(String(message.gameId), userId, type.slice(5));
			return null;
		}
		if (type === 'friend.request') {
			const targetId = String(message.userId);
			if (targetId === userId) throw new Error('Choose another player.');
			if (!(await db.user.findUnique({ where: { id: targetId } })))
				throw new Error('Player not found.');
			const pairKey = [userId, targetId].sort().join(':');
			const existing = await db.friendship.findUnique({ where: { pairKey } });
			if (existing?.status === 'accepted') throw new Error('You are already friends.');
			if (existing && existing.senderId === userId)
				throw new Error('Your friend request is already pending.');
			if (existing)
				await db.friendship.update({ where: { pairKey }, data: { status: 'accepted' } });
			else {
				rateLimit(`friend:${userId}`, 20);
				await db.friendship.create({ data: { pairKey, senderId: userId, recipientId: targetId } });
			}
			await notify([userId, targetId]);
			return null;
		}
		if (type === 'friend.accept' || type === 'friend.remove') {
			const relationship = await db.friendship.findUnique({
				where: { id: String(message.friendshipId) }
			});
			if (!relationship || ![relationship.senderId, relationship.recipientId].includes(userId))
				throw new Error('Friend request not found.');
			if (type === 'friend.accept') {
				if (relationship.recipientId !== userId || relationship.status !== 'pending')
					throw new Error('Only the recipient can accept this request.');
				await db.friendship.update({
					where: { id: relationship.id },
					data: { status: 'accepted' }
				});
			} else await db.friendship.delete({ where: { id: relationship.id } });
			await notify([relationship.senderId, relationship.recipientId]);
			return null;
		}
		if (type === 'challenge.create') {
			rateLimit(`challenge:${userId}`, 10);
			const options = matchOptions(message);
			const recipientId = String(message.userId);
			if (recipientId === userId || !(await db.user.findUnique({ where: { id: recipientId } })))
				throw new Error('Choose another player.');
			const duplicate = await db.challenge.findFirst({
				where: { senderId: userId, recipientId, status: 'pending', expiresAt: { gt: new Date() } }
			});
			if (duplicate) throw new Error('You already have a pending challenge for this player.');
			const challenge = await db.challenge.create({
				data: {
					...options,
					senderId: userId,
					recipientId,
					senderSide: Math.random() < 0.5 ? 1 : 2,
					expiresAt: new Date(Date.now() + 10 * 60_000)
				}
			});
			await notify([userId, recipientId]);
			return { challengeId: challenge.id };
		}
		if (['challenge.accept', 'challenge.decline', 'challenge.cancel'].includes(type)) {
			const challenge = await db.challenge.findUnique({
				where: { id: String(message.challengeId) }
			});
			if (
				!challenge ||
				challenge.status !== 'pending' ||
				challenge.expiresAt.getTime() <= Date.now()
			)
				throw new Error('This challenge is no longer available.');
			if (
				type === 'challenge.cancel'
					? challenge.senderId !== userId
					: challenge.recipientId !== userId
			)
				throw new Error('You cannot change this challenge.');
			if (type === 'challenge.accept') {
				const first = challenge.senderSide === 1 ? challenge.senderId : challenge.recipientId;
				const second = challenge.senderSide === 2 ? challenge.senderId : challenge.recipientId;
				const game = await service.create(
					matchOptions(challenge as unknown as Record<string, unknown>),
					first,
					second,
					null,
					async (tx, gameId) => {
						await tx.challenge.update({
							where: { id: challenge.id },
							data: { status: 'accepted', gameId }
						});
					}
				);
				await created(game.id);
				return { gameId: game.id };
			}
			await db.challenge.update({
				where: { id: challenge.id },
				data: { status: type === 'challenge.decline' ? 'declined' : 'cancelled' }
			});
			await notify([challenge.senderId, challenge.recipientId]);
			return null;
		}
		throw new Error('Unknown request. Refresh the page and try again.');
	}

	function onUpgrade(request: IncomingMessage, socket: Duplex, head: Buffer) {
		if (request.url?.split('?')[0] !== '/ws') return;
		void (async () => {
			try {
				const expectedOrigin = process.env.ORIGIN ?? `http://${request.headers.host}`;
				if (request.headers.origin !== expectedOrigin) throw new Error('Invalid origin');
				rateLimit(`connect:${request.socket.remoteAddress}`, 60);
				const token = readSessionCookie(request.headers.cookie);
				const user = await authenticate(token);
				if ([...clients].filter((client) => user && client.user?.id === user.id).length >= 8)
					throw new Error('Too many connections');
				wss.handleUpgrade(request, socket, head, (ws) => {
					const client: Client = { socket: ws, user, token, watching: null, alive: true };
					clients.add(client);
					ws.on('error', () => ws.terminate());
					ws.on('pong', () => {
						client.alive = true;
					});
					ws.on('message', (raw, binary) => {
						let message: Message;
						try {
							rateLimit(`messages:${user?.id ?? request.socket.remoteAddress}`, 240);
							if (binary) throw new Error('Text messages are required.');
							message = JSON.parse(raw.toString());
							if (
								!message ||
								typeof message.type !== 'string' ||
								typeof message.id !== 'string' ||
								message.id.length > 80
							)
								throw new Error('Invalid request.');
						} catch {
							ws.close(1008, 'Invalid request or too many messages.');
							return;
						}
						void serialize(async () => {
							try {
								send(client, { id: message.id, ok: true, data: await dispatch(client, message) });
							} catch (error) {
								send(client, {
									id: message.id,
									ok: false,
									message: error instanceof Error ? error.message : String(error)
								});
								if (message.type.startsWith('game.') && client.watching) {
									try {
										send(client, {
											type: 'game',
											data: await service.state(client.watching, client.user?.id)
										});
									} catch {
										/* Not a visible game. */
									}
								}
							}
						});
					});
					ws.once('close', () => {
						clients.delete(client);
						void serialize(async () => {
							if (client.user && ![...clients].some((other) => other.user?.id === client.user!.id))
								await db.queueEntry.deleteMany({ where: { userId: client.user.id } });
							if (!stopped) {
								await broadcastLobby();
								await notify(online());
							}
						}).catch(console.error);
					});
					void serialize(async () => {
						await broadcastLobby();
						if (user) await notify(online());
					}).catch(console.error);
				});
			} catch {
				socket.write('HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n');
				socket.destroy();
			}
		})();
	}
	server.on('upgrade', onUpgrade);
	const ready = serialize(async () => {
		await initializeDatabase();
		await db.queueEntry.deleteMany();
		await db.session.deleteMany({ where: { expiresAt: { lt: new Date() } } });
		await service.tick();
	});
	ready.catch((error) => console.error('Realtime initialization failed:', error));
	const interval = setInterval(() => {
		if (stopped || ticking) return;
		ticking = true;
		void serialize(async () => {
			await service.tick();
			await tryMatch();
		})
			.catch(console.error)
			.finally(() => {
				ticking = false;
			});
	}, 1000);
	interval.unref();
	const heartbeat = setInterval(() => {
		for (const client of clients) {
			if (!client.alive) client.socket.terminate();
			else {
				client.alive = false;
				client.socket.ping();
			}
		}
	}, 30_000);
	heartbeat.unref();
	return {
		ready,
		async close() {
			if (stopped) return;
			stopped = true;
			clearInterval(interval);
			clearInterval(heartbeat);
			server.off('upgrade', onUpgrade);
			for (const client of clients) client.socket.close(1001, 'Server restarting');
			await tail;
			await service.close();
			wss.close();
		}
	};
}
