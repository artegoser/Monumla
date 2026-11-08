import { parentPort, workerData } from 'node:worker_threads';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const wasm = require('./wasm/monumla.js');
let game;
try {
	const snapshot = JSON.parse(workerData.snapshot, (_key, item) =>
		item && typeof item === 'object' && Array.isArray(item.__monumlaMap)
			? new Map(item.__monumlaMap)
			: item && typeof item === 'object' && typeof item.__monumlaBigInt === 'string'
				? BigInt(item.__monumlaBigInt)
				: item
	);
	game = wasm.from_search_snapshot(snapshot);
	const level = workerData.level;
	const report = game.analyze(
		level + 1,
		[0, 1000, 5000, 20_000, 75_000][level],
		[0, 100, 200, 400, 700][level],
		1,
		() => {}
	);
	parentPort.postMessage({ action: report.best_action });
} catch (error) {
	parentPort.postMessage({ error: String(error) });
} finally {
	game?.free();
}
