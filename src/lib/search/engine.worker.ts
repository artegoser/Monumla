/// <reference lib="webworker" />
import { LocalGame, LocalSearchSession } from '$lib/wasm/runtime';
import type { SearchMessage, SearchRequest } from './types';

const scope = self as unknown as DedicatedWorkerGlobalScope;
let generation = 0;
let session: LocalSearchSession | null = null;
const elapsedByPosition = new Map<string, number>();

scope.onmessage = ({ data }: MessageEvent<SearchRequest>) => {
	generation = data.id;
	if (!data.cancel) void run(data);
};

async function run(data: SearchRequest) {
	const current = () => generation === data.id;
	const send = (message: SearchMessage) => {
		if (current()) scope.postMessage(message);
	};
	try {
		if (data.ply != null) {
			const game = await LocalGame.fromSearchSnapshot(data.snapshot);
			try {
				if (current())
					send({ id: data.id, type: 'review', review: game.reviewTurn(data.ply, data.settings) });
			} finally {
				game.dispose();
			}
			return;
		}
		if (!session) {
			const created = await LocalSearchSession.create(data.snapshot);
			if (!current()) {
				created.dispose();
				return;
			}
			session = created;
		} else {
			if (!current()) return;
			session.setPosition(data.snapshot);
		}
		const started = performance.now();
		let previousElapsed: number | null = null;
		const publish = (analysis: import('./types').AnalysisView) => {
			const key = analysis.position_key ?? analysis.best_action?.position_key;
			if (key) {
				previousElapsed ??= elapsedByPosition.get(key) ?? 0;
				analysis.elapsed_ms = previousElapsed + performance.now() - started;
				analysis.nodes_per_second = (analysis.nodes * 1000) / Math.max(1, analysis.elapsed_ms);
				if (elapsedByPosition.size > 128) elapsedByPosition.clear();
				elapsedByPosition.set(key, analysis.elapsed_ms);
			}
			if (analysis.score != null || analysis.iteration)
				send({ id: data.id, type: 'progress', analysis });
		};
		if (data.control && session.supportsContinuous) {
			const analysis = session.run(data.settings, new Int32Array(data.control), publish);
			publish(analysis);
			send({ id: data.id, type: 'result', analysis });
			return;
		}
		// Compatibility path for hosts without cross-origin isolation. Search
		// state, root frontiers and sampling batches survive every yield.
		while (current()) {
			const remaining = data.settings.timeMs - (performance.now() - started);
			const analysis = session.chunk(
				data.settings,
				Math.max(20, Math.min(250, remaining)),
				publish
			);
			publish(analysis);
			if (
				analysis.stop_reason === 'terminal' ||
				analysis.stop_reason === 'solved' ||
				analysis.stop_reason === 'forced_move' ||
				analysis.depth >= data.settings.maxDepth ||
				analysis.nodes >= data.settings.maxNodes ||
				performance.now() - started >= data.settings.timeMs
			) {
				send({ id: data.id, type: 'result', analysis });
				return;
			}
			// Yield so cancellation/navigation can replace the root without destroying the engine.
			await new Promise((resolve) => setTimeout(resolve, 0));
		}
	} catch (cause) {
		send({
			id: data.id,
			type: 'error',
			message: cause instanceof Error ? cause.message : String(cause)
		});
	}
}
