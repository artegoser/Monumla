import { Worker } from 'node:worker_threads';
import type { Game, Prisma } from '@prisma/client';
import { db } from './db';
import { createEngine, restoreEngine, encode, wire, type Engine } from './engine';
import { eloChange } from './elo';
import { games, categoryFor, type Ruleset } from '../src/lib/platform/catalog';
import type { BestAction } from '../src/lib/search/types';

export const playerSelect = { id: true, username: true, country: true } as const;
export const gameInclude = {
	firstUser: { select: playerSelect },
	secondUser: { select: playerSelect }
} as const;
export type GameRecord = Prisma.GameGetPayload<{ include: typeof gameInclude }>;
interface Room {
	record: GameRecord;
	engine: Engine;
	touched: number;
}
export interface MatchOptions {
	ruleset: Ruleset;
	initialSeconds: number;
	increment: number;
	rated: boolean;
}

export function matchOptions(value: Record<string, unknown>): MatchOptions {
	const ruleset = String(value.ruleset);
	if (!games.some((game) => game.id === ruleset)) throw new Error('Choose a supported game.');
	const initialSeconds = Number(value.initialSeconds);
	const increment = Number(value.increment);
	if (
		!Number.isInteger(initialSeconds) ||
		initialSeconds < 60 ||
		initialSeconds > 10_800 ||
		!Number.isInteger(increment) ||
		increment < 0 ||
		increment > 180
	)
		throw new Error('Choose 1–180 minutes and 0–180 seconds of increment.');
	if (typeof value.rated !== 'boolean') throw new Error('Choose rated or casual play.');
	return { ruleset: ruleset as Ruleset, initialSeconds, increment, rated: value.rated };
}

export function sideFor(game: Pick<Game, 'firstUserId' | 'secondUserId'>, userId?: string | null) {
	return userId && game.firstUserId === userId ? 1 : userId && game.secondUserId === userId ? 2 : 0;
}

export function clocks(game: Game, now = Date.now()) {
	const elapsed = game.status === 'active' ? Math.max(0, now - game.turnStartedAt.getTime()) : 0;
	return {
		firstMs: Math.max(0, game.firstMs - (game.activeSide === 1 ? elapsed : 0)),
		secondMs: Math.max(0, game.secondMs - (game.activeSide === 2 ? elapsed : 0)),
		serverTime: now
	};
}

export function gameSummary(game: GameRecord) {
	return {
		id: game.id,
		ruleset: game.ruleset,
		category: game.category,
		initialSeconds: game.initialSeconds,
		increment: game.increment,
		rated: game.rated,
		status: game.status,
		result: game.result,
		termination: game.termination,
		computerLevel: game.computerLevel,
		firstUser: game.firstUser,
		secondUser: game.secondUser,
		firstRating: game.firstRating,
		secondRating: game.secondRating,
		firstDelta: game.firstDelta,
		secondDelta: game.secondDelta,
		createdAt: game.createdAt.toISOString(),
		finishedAt: game.finishedAt?.toISOString() ?? null
	};
}

export class GameService {
	private rooms = new Map<string, Room>();
	private bots = new Set<string>();
	private workers = new Set<Worker>();
	private botJobs = new Set<Promise<unknown>>();
	private closing = false;
	onChange: (id: string) => Promise<void> = async () => {};
	serialize: <T>(task: () => Promise<T>) => Promise<T> = (task) => task();

	async room(id: string) {
		let room = this.rooms.get(id);
		if (!room) {
			const record = await db.game.findUnique({ where: { id }, include: gameInclude });
			if (!record) throw new Error('This game could not be found.');
			room = { record, engine: restoreEngine(record.snapshot), touched: Date.now() };
			this.rooms.set(id, room);
		}
		room.touched = Date.now();
		return room;
	}

	async create(
		options: MatchOptions,
		firstUserId: string | null,
		secondUserId: string | null,
		computerLevel: number | null = null,
		extra?: (tx: Prisma.TransactionClient, id: string) => Promise<void>
	) {
		options = matchOptions(options as unknown as Record<string, unknown>);
		if ((!firstUserId && !secondUserId) || firstUserId === secondUserId)
			throw new Error('A match needs two different players.');
		if (
			computerLevel !== null &&
			(!Number.isInteger(computerLevel) || computerLevel < 1 || computerLevel > 4)
		)
			throw new Error('Choose computer level 1–4.');
		const engine = createEngine(options.ruleset);
		try {
			const record = await db.$transaction(async (tx) => {
				const ids = [firstUserId, secondUserId].filter((id): id is string => !!id);
				const existing = await tx.game.findFirst({
					where: {
						status: 'active',
						OR: [{ firstUserId: { in: ids } }, { secondUserId: { in: ids } }]
					}
				});
				if (existing)
					throw new Error('One of the players already has an active game. Finish it first.');
				const category = categoryFor(options.initialSeconds, options.increment);
				const ratings = await tx.rating.findMany({
					where: { userId: { in: ids }, ruleset: options.ruleset, category }
				});
				const record = await tx.game.create({
					data: {
						...options,
						rated: options.rated && !!firstUserId && !!secondUserId,
						category,
						firstUserId,
						secondUserId,
						computerLevel,
						firstRating: ratings.find((rating) => rating.userId === firstUserId)?.value ?? 1500,
						secondRating: ratings.find((rating) => rating.userId === secondUserId)?.value ?? 1500,
						firstMs: options.initialSeconds * 1000,
						secondMs: options.initialSeconds * 1000,
						activeSide: engine.view().active_players[0],
						snapshot: encode(engine.exportState())
					},
					include: gameInclude
				});
				engine.setPerspective(1);
				const viewOne = encode(wire(engine.view()));
				engine.setPerspective(2);
				const viewTwo = encode(wire(engine.view()));
				await tx.gameMove.create({
					data: {
						gameId: record.id,
						ply: 0,
						actor: 0,
						notation: 'Starting position',
						firstMs: record.firstMs,
						secondMs: record.secondMs,
						viewOne,
						viewTwo
					}
				});
				await tx.queueEntry.deleteMany({ where: { userId: { in: ids } } });
				if (extra) await extra(tx, record.id);
				return record;
			});
			this.rooms.set(record.id, { record, engine, touched: Date.now() });
			return record;
		} catch (error) {
			engine.free();
			throw error;
		}
	}

	async state(id: string, userId?: string | null) {
		const room = await this.room(id);
		const { record, engine } = room;
		const side = sideFor(record, userId);
		if (!side && record.ruleset === 'battleship' && record.status === 'active')
			throw new Error('Battleship spectating is available after the game to protect both fleets.');
		engine.setPerspective(side || 1);
		const view = engine.view();
		const interactive = record.status === 'active' && side === record.activeSide;
		return wire({
			...gameSummary(record),
			...clocks(record),
			side,
			revision: record.revision,
			activeSide: record.activeSide,
			drawOffer: record.drawOffer,
			view,
			interaction: interactive
				? engine.interaction()
				: {
						generation: '',
						selected_entity: null,
						pending_target: null,
						active_ability: null,
						choices: []
					},
			history: engine.history().map(({ index, actor, notation }) => ({ index, actor, notation }))
		});
	}

	async play(id: string, userId: string, choice: string | null, revision: number) {
		const room = await this.room(id);
		const side = sideFor(room.record, userId);
		if (!side || side !== room.record.activeSide || room.record.status !== 'active')
			throw new Error('It is not your turn.');
		if (revision !== room.record.revision)
			throw new Error('The position changed. Your board has been refreshed.');
		if (await this.timeout(room)) return;
		room.engine.setPerspective(side);
		try {
			if (choice === null) room.engine.cancelSelection();
			else {
				if (typeof choice !== 'string' || choice.length > 100)
					throw new Error('Choose a legal move.');
				const transition = room.engine.choose(choice);
				if (transition.committed) await this.commit(room);
			}
		} catch (error) {
			// Rehydrate the last committed turn if a write or runtime operation failed.
			room.engine.free();
			this.rooms.delete(id);
			throw error;
		}
		await this.onChange(id);
	}

	private async commit(room: Room) {
		const { record, engine } = room;
		const now = Date.now();
		const time = clocks(record, now);
		const view = engine.view();
		const history = engine.history();
		const turn = history[history.length - 1];
		if (!turn) throw new Error('The move did not produce a turn.');
		const firstMs = time.firstMs + (record.activeSide === 1 ? record.increment * 1000 : 0);
		const secondMs = time.secondMs + (record.activeSide === 2 ? record.increment * 1000 : 0);
		engine.setPerspective(1);
		const viewOne = encode(wire(engine.view()));
		engine.setPerspective(2);
		const viewTwo = encode(wire(engine.view()));
		await db.$transaction(async (tx) => {
			await tx.gameMove.create({
				data: {
					gameId: record.id,
					ply: history.length,
					actor: turn.actor,
					notation: turn.notation,
					firstMs,
					secondMs,
					viewOne,
					viewTwo
				}
			});
			await tx.game.update({
				where: { id: record.id },
				data: {
					snapshot: encode(engine.exportState()),
					revision: { increment: 1 },
					activeSide: view.active_players[0] ?? record.activeSide,
					firstMs,
					secondMs,
					turnStartedAt: new Date(now),
					drawOffer: null
				}
			});
		});
		room.record = await db.game.findUniqueOrThrow({
			where: { id: record.id },
			include: gameInclude
		});
		if (view.status.outcome) {
			const winners = view.status.outcome.winners;
			await this.finish(
				room,
				winners.includes(1) ? '1-0' : winners.includes(2) ? '0-1' : '1/2-1/2',
				view.status.outcome.key
			);
		}
	}

	private async finish(room: Room, result: string, termination: string) {
		if (room.record.status !== 'active') return;
		const { record } = room;
		const time = clocks(record);
		await db.$transaction(async (tx) => {
			const claimed = await tx.game.updateMany({
				where: { id: record.id, status: 'active' },
				data: {
					status: 'finished',
					result,
					termination,
					finishedAt: new Date(),
					firstMs: time.firstMs,
					secondMs: time.secondMs,
					revision: { increment: 1 },
					drawOffer: null
				}
			});
			if (!claimed.count || !record.rated || !record.firstUserId || !record.secondUserId) return;
			const score = result === '1-0' ? 1 : result === '0-1' ? 0 : 0.5;
			const deltas: number[] = [];
			for (const [userId, mine, theirs, outcome] of [
				[record.firstUserId, record.firstRating, record.secondRating, score],
				[record.secondUserId, record.secondRating, record.firstRating, 1 - score]
			] as const) {
				const key = { userId, ruleset: record.ruleset, category: record.category };
				const rating = await tx.rating.upsert({
					where: { userId_ruleset_category: key },
					create: key,
					update: {}
				});
				const delta = eloChange(mine, theirs, outcome, rating.games);
				deltas.push(delta);
				await tx.rating.update({
					where: { id: rating.id },
					data: {
						value: { increment: delta },
						games: { increment: 1 },
						wins: { increment: outcome === 1 ? 1 : 0 },
						draws: { increment: outcome === 0.5 ? 1 : 0 },
						losses: { increment: outcome === 0 ? 1 : 0 }
					}
				});
			}
			await tx.game.update({
				where: { id: record.id },
				data: { firstDelta: deltas[0], secondDelta: deltas[1] }
			});
		});
		room.record = await db.game.findUniqueOrThrow({
			where: { id: record.id },
			include: gameInclude
		});
	}

	async adjudicate(id: string, userId: string, action: string) {
		const room = await this.room(id);
		const side = sideFor(room.record, userId);
		if (!side || room.record.status !== 'active')
			throw new Error('This game has already ended or you are not a player.');
		if (await this.timeout(room)) return;
		if (action === 'resign') await this.finish(room, side === 1 ? '0-1' : '1-0', 'resignation');
		else if (action === 'draw.accept') {
			if (!room.record.drawOffer || room.record.drawOffer === side)
				throw new Error('There is no draw offer to accept.');
			await this.finish(room, '1/2-1/2', 'draw agreement');
		} else if (action === 'draw') {
			if (room.record.computerLevel) throw new Error('The computer does not accept draw offers.');
			room.record = await db.game.update({
				where: { id },
				data: { drawOffer: side },
				include: gameInclude
			});
		} else if (action === 'draw.decline') {
			if (!room.record.drawOffer || room.record.drawOffer === side)
				throw new Error('There is no draw offer to decline.');
			room.record = await db.game.update({
				where: { id },
				data: { drawOffer: null },
				include: gameInclude
			});
		} else throw new Error('Unknown game action.');
		await this.onChange(id);
	}

	private async timeout(room: Room) {
		if (room.record.status !== 'active') return false;
		const time = clocks(room.record);
		if (time.firstMs > 0 && time.secondMs > 0) return false;
		await this.finish(room, time.firstMs <= 0 ? '0-1' : '1-0', 'timeout');
		await this.onChange(room.record.id);
		return true;
	}

	async tick() {
		if (this.closing) return;
		const active = await db.game.findMany({ where: { status: 'active' }, select: { id: true } });
		for (const { id } of active) {
			const room = await this.room(id);
			if (await this.timeout(room)) continue;
			if (
				room.record.computerLevel &&
				!this.bots.has(id) &&
				this.bots.size < 2 &&
				!(room.record.activeSide === 1 ? room.record.firstUserId : room.record.secondUserId)
			)
				this.startBot(room);
		}
		for (const [id, room] of this.rooms)
			if (room.record.status === 'finished' && Date.now() - room.touched > 60_000) {
				room.engine.free();
				this.rooms.delete(id);
			}
	}

	private startBot(room: Room) {
		const { record, engine } = room;
		this.bots.add(record.id);
		engine.setPerspective(record.activeSide);
		const worker = new Worker(new URL('./bot-worker.mjs', import.meta.url), {
			workerData: { snapshot: encode(engine.searchSnapshot()), level: record.computerLevel }
		});
		this.workers.add(worker);
		const timer = setTimeout(() => void worker.terminate(), 10_000);
		let responded = false;
		worker.once('message', (message: { action?: BestAction; error?: string }) => {
			responded = true;
			if (this.closing) {
				this.bots.delete(record.id);
				return;
			}
			const job = this.serialize(async () => {
				if (this.closing) return;
				const latest = await this.room(record.id);
				if (
					this.closing ||
					latest.record.status !== 'active' ||
					latest.record.revision !== record.revision
				)
					return;
				if (await this.timeout(latest)) return;
				if (!message.action)
					throw new Error(message.error ?? 'The computer could not choose a move.');
				latest.engine.applyBestAction(message.action);
				await this.commit(latest);
				await this.onChange(record.id);
			})
				.catch((error) => console.error('Computer move failed:', error))
				.finally(() => this.bots.delete(record.id));
			this.botJobs.add(job);
			void job.finally(() => this.botJobs.delete(job));
		});
		worker.once('error', (error) => console.error('Computer worker failed:', error));
		worker.once('exit', () => {
			this.workers.delete(worker);
			clearTimeout(timer);
			if (!responded) this.bots.delete(record.id);
		});
	}

	async close() {
		this.closing = true;
		await Promise.allSettled([...this.workers].map((worker) => worker.terminate()));
		await Promise.allSettled(this.botJobs);
		for (const room of this.rooms.values()) room.engine.free();
		this.rooms.clear();
	}
}
