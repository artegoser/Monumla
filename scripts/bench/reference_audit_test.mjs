#!/usr/bin/env node
// Integration checks against real frozen rules, including dishonest PGN headers.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { replayResult } from './reference_audit.mjs';
const root = process.argv[2];
if (!root) throw new Error('Usage: reference_audit_test.mjs FROZEN_WASM_DIRECTORY');
const module = await import(pathToFileURL(path.resolve(root, 'monumla.js')));
await module.default({ module_or_path: fs.readFileSync(path.resolve(root, 'monumla_bg.wasm')) });
const pgn = (fen, moves = '1/2-1/2') =>
	`[SetUp "1"]\n[FEN "${fen}"]\n[Result "${moves.trim().split(/\s+/).at(-1)}"]\n\n${moves}`;
const row = { side: '1', score: '0.5', termination: 'chess.fifty_move_rule' };
for (const [halfmove, byMove] of [
	[99, true],
	[100, false]
]) {
	const { game, reconstruction } = replayResult(
		module,
		pgn(`7k/8/8/8/8/8/R7/K7 w - - ${halfmove} 51`),
		row
	);
	assert.equal(reconstruction.by_move, byMove);
	assert.equal(game.view().status.outcome.key, 'chess.fifty_move_rule');
	game.free();
}
assert.throws(
	() => replayResult(module, pgn('7k/8/8/8/8/8/R7/K7 w - - 0 1'), row),
	/independently legal/
);
const repetition = '[Result "1/2-1/2"]\n\n1. Nf3 Nf6 2. Ng1 Ng8 3. Nf3 Nf6 4. Ng1 1/2-1/2';
const repeated = replayResult(module, repetition, {
	side: '2',
	score: '0.5',
	termination: 'chess.threefold_repetition'
});
assert.equal(repeated.reconstruction.by_move, true);
assert.equal(repeated.reconstruction.san, 'Ng8');
repeated.game.free();
assert.throws(
	() =>
		replayResult(module, repetition, {
			side: '1',
			score: '0.5',
			termination: 'chess.threefold_repetition'
		}),
	/Only Monumla/
);
const mate = replayResult(module, pgn('7k/6Q1/5K2/8/8/8/8/8 b - - 0 1', '1-0'), {
	side: '1',
	score: '1',
	termination: 'chess.checkmate'
});
assert.equal(mate.reconstruction, null);
mate.game.free();
assert.throws(() =>
	replayResult(module, pgn('7k/8/8/8/8/8/R7/K7 w - - 0 1', '1-0'), {
		side: '1',
		score: '1',
		termination: 'chess.checkmate'
	})
);
console.log('Seven frozen-WASM result/claim audit checks passed.');
