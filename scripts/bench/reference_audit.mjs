#!/usr/bin/env node
// Recheck completed calibration PGNs with the frozen public rules API.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';

const [wasmDirectory, runDirectory] = process.argv.slice(2);
if (!wasmDirectory || !runDirectory) {
	throw new Error('Usage: reference_audit.mjs WASM_DIRECTORY RUN_DIRECTORY');
}
const root = path.resolve(runDirectory);
const modulePath = path.resolve(wasmDirectory, 'monumla.js');
const wasmPath = path.resolve(wasmDirectory, 'monumla_bg.wasm');
const module = await import(pathToFileURL(modulePath));
const bytes = fs.readFileSync(wasmPath);
await module.default({ module_or_path: bytes });
const manifest = JSON.parse(fs.readFileSync(path.join(root, 'manifest.json'), 'utf8'));
const lines = fs.readFileSync(path.join(root, 'games.tsv'), 'utf8').trim().split(/\r?\n/);
const columns = lines.shift().split('\t');
const rows = lines.map((line) =>
	Object.fromEntries(line.split('\t').map((v, i) => [columns[i], v]))
);
const scalar = (details, key) => (details instanceof Map ? details.get(key) : details[key])?.value;
const records = [];
for (const row of rows) {
	const gameNumber = String(row.game).padStart(3, '0');
	const directory = path.join(root, 'moves', `game-${gameNumber}`);
	const settings = Object.fromEntries(
		fs
			.readFileSync(path.join(directory, 'settings.tsv'), 'utf8')
			.trim()
			.split(/\r?\n/)
			.map((line) => line.split('\t'))
	);
	assert.equal(settings.elo, `Some(${manifest.baseline_settings.elo})`);
	assert.equal(settings.clock_ms, `Some(${manifest.baseline_settings.clock_ms})`);
	assert.equal(Number(settings.increment_ms), manifest.baseline_settings.increment_ms);
	const game = module.from_pgn(
		fs.readFileSync(path.join(directory, `game-${gameNumber}.pgn`), 'utf8')
	);
	try {
		const final = game.view();
		assert.equal(final.status.outcome?.key, row.termination);
		if (row.score === '0.5') assert.equal(final.status.outcome.winners.length, 0);
		else {
			assert.ok(['0', '1'].includes(row.score), 'Audit requires a completed rules result');
			assert.equal(final.status.outcome.winners.includes(Number(row.side)), row.score === '1');
		}
		let positions = 0;
		const claims = [];
		while (true) {
			const view = game.view();
			const threefold = scalar(view.status.details, 'can_claim_threefold_repetition');
			const fiftyMove = scalar(view.status.details, 'can_claim_fifty_move_rule');
			assert.equal(typeof threefold, 'boolean');
			assert.equal(typeof fiftyMove, 'boolean');
			if (!view.status.outcome && (threefold || fiftyMove)) {
				claims.push({
					ply: game.history().length,
					active_players: view.active_players,
					threefold,
					fiftyMove
				});
			}
			positions++;
			if (!game.canUndo()) break;
			game.undo();
		}
		records.push({ game: Number(row.game), positions, claims });
	} finally {
		game.free();
	}
}
const result = {
	audited_games: records.length,
	planned_games: manifest.games,
	complete_schedule: records.length === manifest.games,
	wasm_sha256: createHash('sha256').update(bytes).digest('hex'),
	records
};
const output = path.join(root, 'completed-games-rules-audit.json');
fs.writeFileSync(output + '.next', JSON.stringify(result, null, 2) + '\n');
fs.renameSync(output + '.next', output);
console.log(
	JSON.stringify({
		audited_games: records.length,
		positions: records.reduce((sum, r) => sum + r.positions, 0),
		claimable_positions: records.reduce((sum, r) => sum + r.claims.length, 0)
	})
);
