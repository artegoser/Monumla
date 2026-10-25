#!/usr/bin/env node
/** Compare frozen WASM builds on the native matrix's identical public snapshots. */
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { cpus } from 'node:os';

const args = Object.fromEntries(
	Array.from({ length: (process.argv.length - 2) / 2 }, (_, i) => [
		process.argv[2 + i * 2].replace(/^--/, ''),
		process.argv[3 + i * 2]
	])
);
for (const name of ['baseline', 'candidate', 'matrix', 'out'])
	if (!args[name]) throw new Error(`Missing --${name}`);
const repeats = Number(args.repeats ?? 5);
if (!Number.isInteger(repeats) || repeats < 3)
	throw new Error('Require at least three measured repetitions');
await mkdir(args.out); // Never overwrite existing runs.
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
const matrix = JSON.parse(await readFile(join(args.matrix, 'matrix.json'), 'utf8'));
const modules = {},
	hashes = {};
for (const side of ['baseline', 'candidate']) {
	const bytes = await readFile(join(args[side], 'monumla_bg.wasm'));
	modules[side] = await import(pathToFileURL(resolve(args[side], 'monumla.js')).href);
	await modules[side].default({ module_or_path: bytes });
	hashes[side] = hash(bytes);
}
function hydrate(value) {
	if (!value || typeof value !== 'object') return value;
	if (Array.isArray(value)) return value.map(hydrate);
	const result = Object.fromEntries(
		Object.entries(value).map(([key, item]) => [key, hydrate(item)])
	);
	if (result.board && result.entities && result.players && result.turn && result.ruleset_state) {
		for (const key of ['entities', 'players', 'teams'])
			result[key] = new Map(Object.entries(result[key]).map(([id, item]) => [Number(id), item]));
	}
	return result;
}
const serialize = (value) =>
	JSON.stringify(value, (_, item) => (typeof item === 'bigint' ? item.toString() : item));
const identity = (report) =>
	Object.fromEntries(
		['depth', 'nodes', 'score', 'principal_variation', 'lines', 'stop_reason'].map((key) => [
			key,
			report[key]
		])
	);
function timing(values) {
	const sorted = [...values].sort((a, b) => a - b);
	return {
		median_ms: sorted[Math.floor(sorted.length / 2)],
		min_ms: sorted[0],
		max_ms: sorted.at(-1)
	};
}
const output = {
	settings: args,
	source_revisions: {
		baseline: matrix.settings.baseline_revision,
		candidate: matrix.settings.candidate_revision
	},
	runtime: process.versions,
	cpu: cpus()[0]?.model,
	wasm_sha256: hashes,
	results: []
};
for (const { case: fixture } of matrix.results) {
	const snapshotBytes = await readFile(join(args.matrix, 'snapshots', `${fixture.id}.json`));
	const snapshot = hydrate(JSON.parse(snapshotBytes));
	const records = [],
		times = { baseline: [], candidate: [] },
		answers = [];
	const row = { case: fixture, snapshot_sha256: hash(snapshotBytes), records };
	try {
		for (let iteration = 0; iteration <= repeats; iteration++) {
			const results = {};
			for (const side of iteration % 2 ? ['candidate', 'baseline'] : ['baseline', 'candidate']) {
				const game = modules[side].from_search_snapshot(snapshot);
				try {
					let callbacks = 0;
					const start = performance.now();
					const report = game.analyze(fixture.depth, fixture.nodes, 600000, 1, (value) => {
						callbacks++;
						structuredClone(value);
					});
					const elapsed_ms = performance.now() - start;
					records.push({ side, iteration, warmup: iteration === 0, elapsed_ms, callbacks, report });
					if (iteration) times[side].push(elapsed_ms);
					results[side] = serialize(identity(report));
				} finally {
					game.free();
				}
			}
			answers.push(results.baseline === results.candidate);
		}
		Object.assign(row, {
			same_work_and_answer: answers.every(Boolean),
			baseline: timing(times.baseline),
			candidate: timing(times.candidate)
		});
		row.speedup = row.baseline.median_ms / row.candidate.median_ms;
	} catch (error) {
		row.error = String(error?.stack ?? error);
	}
	output.results.push(row);
	await writeFile(join(args.out, 'matrix.json'), serialize(output));
	console.log(serialize({ ...row, records: undefined }));
}
if (output.results.some((row) => row.error || !row.same_work_and_answer)) process.exitCode = 1;
