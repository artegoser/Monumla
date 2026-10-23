/// <reference lib="webworker" />
import { LocalGame } from '$lib/wasm/runtime';
import type { SearchMessage, SearchRequest } from './types';

const scope = self as unknown as DedicatedWorkerGlobalScope;
scope.onmessage = async ({ data }: MessageEvent<SearchRequest>) => {
	let game: LocalGame | null = null;
	const send = (message: SearchMessage) => scope.postMessage(message);
	try {
		game = await LocalGame.fromSearchSnapshot(data.snapshot);
		if (data.ply != null) {
			const review = game.reviewTurn(data.ply, data.settings);
			send({ id: data.id, type: 'review', review });
			return;
		}
		const analysis = game.analyze(data.settings, (analysis) =>
			send({ id: data.id, type: 'progress', analysis })
		);
		send({ id: data.id, type: 'result', analysis });
	} catch (cause) {
		send({
			id: data.id,
			type: 'error',
			message: cause instanceof Error ? cause.message : String(cause)
		});
	} finally {
		game?.dispose();
	}
};
