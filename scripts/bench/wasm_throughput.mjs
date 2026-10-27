#!/usr/bin/env node
/** Fixed-position V8/WASM probe, including live report rendering and chunk resumption. */
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
const args = Object.fromEntries(Array.from({ length: (process.argv.length - 2) / 2 }, (_, i) => [process.argv[2 + i * 2].replace(/^--/, ''), process.argv[3 + i * 2]]));
if (!args.module || !args.out) throw new Error('Use --module WASM_DIR --out NEW_JSON [--position PGN_OR_go:19] [--depth 3] [--lines 3] [--repeats 5] [--budget-ms 15000]');
try { await readFile(args.out); throw new Error('Preserve existing measurements'); } catch (e) { if (e.code !== 'ENOENT') throw e; }
const bytes = await readFile(resolve(args.module, 'monumla_bg.wasm'));
const wasm = await import(pathToFileURL(resolve(args.module, 'monumla.js')).href);
await wasm.default({ module_or_path: bytes });
function hydrate(value) {
 if (!value || typeof value !== 'object') return value;
 if (Array.isArray(value)) return value.map(hydrate);
 const result = Object.fromEntries(Object.entries(value).map(([key,item])=>[key,hydrate(item)]));
 if (result.board && result.entities && result.players && result.turn && result.ruleset_state) {
  for(const key of ['entities','players','teams']) result[key] = new Map(Object.entries(result[key]).map(([id,item])=>[Number(id),item]));
 }
 return result;
}
const depth = Number(args.depth ?? 3), lines = Number(args.lines ?? 3), repeats = Number(args.repeats ?? 5), budget = Number(args['budget-ms'] ?? 15000);
const records = [];
for (let trial = 0; trial <= repeats; trial++) {
 const game = args.snapshot ? wasm.from_search_snapshot(hydrate(JSON.parse(await readFile(args.snapshot, 'utf8')))) : args.position?.startsWith('go:') ? wasm.new_go(Number(args.position.slice(3)), 'area', 0) : args.position ? wasm.from_pgn(await readFile(args.position, 'utf8')) : wasm.new_chess();
 if (args['opening-plies']) for (let ply = 0; ply < Number(args['opening-plies']); ply++) {
  const choices = game.interaction().choices.filter(c => c.kind === 'select_position');
  if (!choices.length) throw new Error('No legal placement in generated prefix');
  game.choose(choices[(ply * 104729 + 7919) % choices.length].id);
 }
 const session = new wasm.SearchSession(game.searchSnapshot());
 let callbacks = 0, chunks = 0, report;
 const start = performance.now();
 try {
  do {
   const progress = value => { callbacks++; structuredClone(value); };
   report = args.continuous === 'true'
    ? session.runWithControl(depth, 50000000, budget, lines, false, new Int32Array(new SharedArrayBuffer(4)), progress)
    : session.chunkWithProgress(depth, 50000000, 250, lines, false, progress);
   chunks++;
  } while (!['solved', 'terminal', 'forced_move'].includes(report.stop_reason) && report.depth < depth && performance.now() - start < budget);
  const elapsed_ms = performance.now() - start;
  const row = { trial, warmup: trial === 0, elapsed_ms, nps: Number(report.nodes) * 1000 / elapsed_ms, chunks, callbacks, report };
  records.push(row); console.log(JSON.stringify({ trial, elapsed_ms, nps: row.nps, depth: report.depth, nodes: String(report.nodes), chunks, callbacks }));
 } finally { session.free(); game.free(); }
}
const elapsed = records.slice(1).map(r => r.elapsed_ms).sort((a, b) => a - b);
const output = { settings: args, runtime: process.versions, wasm_sha256: createHash('sha256').update(bytes).digest('hex'), median_ms: elapsed[Math.floor(elapsed.length / 2)], records };
await mkdir(dirname(resolve(args.out)), { recursive: true });
await writeFile(args.out, JSON.stringify(output, (_, value) => typeof value === 'bigint' ? value.toString() : value, 2));
