#!/usr/bin/env node
// Recheck completed calibration PGNs with the frozen public rules API.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';

const scalar = (details, key) => (details instanceof Map ? details.get(key) : details[key])?.value;
const claimKinds = {
	'chess.fifty_move_rule': ['fifty_move_rule', 'current_fifty_move_rule'],
	'chess.threefold_repetition': ['threefold_repetition', 'current_threefold_repetition']
};

export function replayResult(module, pgn, row) {
	// A result token can manufacture a recorded outcome on PGN import. Replay
	// the actual moves first; a matching header alone is never proof of a draw.
	const moves = claimKinds[row.termination]
		? pgn
				.replace(/^\[Result .*\]$/m, '[Result "*"]')
				.replace(/^\[Termination .*\]\r?\n/m, '')
				.replace(/(?:1\/2-1\/2|1-0|0-1|\*)\s*$/, '*')
		: pgn;
	const game = module.from_pgn(moves);
	let reconstruction = null;
	try {
		if (claimKinds[row.termination]) {
			assert.equal(row.score, '0.5');
			assert.equal(game.view().status.outcome, undefined);
			assert.ok(
				game.view().active_players.includes(Number(row.side)),
				'Only Monumla can submit a draw claim through this host'
			);
			const [kind, current] = claimKinds[row.termination];
			const claims = game.chessDrawClaims();
			if (claims[current]) {
				game.chessClaimDraw(kind);
				reconstruction = { kind, by_move: false };
			} else {
				const witness = claims.by_move.find((move) => move.kind === kind);
				assert.ok(witness, 'Recorded claim must have an independently legal intended move');
				game.chessClaimDrawAfterSan(kind, witness.san);
				reconstruction = {
					kind,
					by_move: true,
					san: witness.san,
					note: 'PGN omits the original intended move; this is an independently legal witness, not a recovered move.'
				};
			}
		}
		const final = game.view();
		assert.equal(final.status.outcome?.key, row.termination);
		if (row.score === '0.5') assert.equal(final.status.outcome.winners.length, 0);
		else {
			assert.ok(['0', '1'].includes(row.score), 'Audit requires a completed rules result');
			assert.equal(final.status.outcome.winners.includes(Number(row.side)), row.score === '1');
		}
		return { game, reconstruction };
	} catch (error) {
		game.free();
		throw error;
	}
}

export function auditRun(module, root, wasmBytes) {
	const manifest = JSON.parse(fs.readFileSync(path.join(root, 'manifest.json'), 'utf8'));
	const lines = fs.readFileSync(path.join(root, 'games.tsv'), 'utf8').trim().split(/\r?\n/);
	const columns = lines.shift().split('\t');
	const rows = lines.map((line) =>
		Object.fromEntries(line.split('\t').map((v, i) => [columns[i], v]))
	);
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
		const pgn = fs.readFileSync(path.join(directory, `game-${gameNumber}.pgn`), 'utf8');
		const { game, reconstruction } = replayResult(module, pgn, row);
		try {
			let positions = 0;
			const claims = [];
			while (true) {
				const view = game.view();
				const threefold = scalar(view.status.details, 'can_claim_threefold_repetition');
				const fiftyMove = scalar(view.status.details, 'can_claim_fifty_move_rule');
				assert.equal(typeof threefold, 'boolean');
				assert.equal(typeof fiftyMove, 'boolean');
				if (!view.status.outcome) {
					const available = game.chessDrawClaims();
					assert.equal(available.current_threefold_repetition, threefold);
					assert.equal(available.current_fifty_move_rule, fiftyMove);
					if (threefold || fiftyMove || available.by_move.length) {
						claims.push({
							ply: game.history().length,
							active_players: view.active_players,
							threefold,
							fiftyMove,
							by_move: available.by_move
						});
					}
				}
				positions++;
				if (!game.canUndo()) break;
				game.undo();
			}
			records.push({
				game: Number(row.game),
				positions,
				claims,
				claim_reconstruction: reconstruction
			});
		} finally {
			game.free();
		}
	}
	const result = {
		audited_games: records.length,
		planned_games: manifest.games,
		complete_schedule: records.length === manifest.games,
		wasm_sha256: createHash('sha256').update(wasmBytes).digest('hex'),
		records
	};
	const output = path.join(root, 'completed-games-rules-audit.json');
	fs.writeFileSync(output + '.next', JSON.stringify(result, null, 2) + '\n');
	fs.renameSync(output + '.next', output);
	return {
		audited_games: records.length,
		positions: records.reduce((sum, r) => sum + r.positions, 0),
		claimable_positions: records.reduce((sum, r) => sum + r.claims.length, 0)
	};
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
	const [wasmDirectory, runDirectory] = process.argv.slice(2);
	if (!wasmDirectory || !runDirectory)
		throw new Error('Usage: reference_audit.mjs WASM_DIRECTORY RUN_DIRECTORY');
	const module = await import(pathToFileURL(path.resolve(wasmDirectory, 'monumla.js')));
	const bytes = fs.readFileSync(path.resolve(wasmDirectory, 'monumla_bg.wasm'));
	await module.default({ module_or_path: bytes });
	console.log(JSON.stringify(auditRun(module, path.resolve(runDirectory), bytes)));
}
