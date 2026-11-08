import { createRequire } from 'node:module';
import { randomInt } from 'node:crypto';
import type {
	GameView,
	InteractionView,
	HistoryTurnView,
	TransitionView
} from '../src/lib/wasm/runtime';
import type { BestAction } from '../src/lib/search/types';
import type { Ruleset } from '../src/lib/platform/catalog';

export interface Engine {
	setPerspective(side: number): void;
	view(): GameView;
	interaction(): InteractionView;
	history(): HistoryTurnView[];
	choose(choice: string): TransitionView;
	cancelSelection(): TransitionView;
	exportState(): unknown;
	searchSnapshot(): unknown;
	applyBestAction(action: BestAction): TransitionView;
	free(): void;
}
interface Module {
	new_game(id: string): Engine;
	new_battleship(seed: number): Engine;
	from_search_snapshot(snapshot: unknown): Engine;
}
const require = createRequire(import.meta.url);
let module: Module;
function wasm() {
	return (module ??= require('./wasm/monumla.js') as Module);
}
export function createEngine(ruleset: Ruleset) {
	return ruleset === 'battleship'
		? wasm().new_battleship(randomInt(0, 0x1_0000_0000))
		: wasm().new_game(ruleset);
}

// Preserve typed maps, including numeric keys, across SQLite and worker messages.
export function encode(value: unknown): string {
	return JSON.stringify(value, (_key, item) =>
		item instanceof Map
			? { __monumlaMap: [...item] }
			: typeof item === 'bigint'
				? { __monumlaBigInt: item.toString() }
				: item
	);
}
export function decode(value: string): unknown {
	return JSON.parse(value, (_key, item) =>
		item && typeof item === 'object' && Array.isArray(item.__monumlaMap)
			? new Map(item.__monumlaMap)
			: item && typeof item === 'object' && typeof item.__monumlaBigInt === 'string'
				? BigInt(item.__monumlaBigInt)
				: item
	);
}
export function restoreEngine(snapshot: string) {
	return wasm().from_search_snapshot(decode(snapshot));
}
export function wire<T>(value: T): T {
	return JSON.parse(
		JSON.stringify(value, (_key, item) =>
			item instanceof Map
				? Object.fromEntries(item)
				: typeof item === 'bigint'
					? Number(item)
					: item
		)
	);
}
