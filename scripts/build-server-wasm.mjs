import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';

const result = spawnSync(
	'wasm-pack',
	[
		'build',
		'crates/monumla-wasm',
		'--target',
		'nodejs',
		'--out-dir',
		'../../server/wasm',
		'--out-name',
		'monumla'
	],
	{ stdio: 'inherit' }
);
if (result.status !== 0) process.exit(result.status ?? 1);
const path = new URL('../server/wasm/package.json', import.meta.url);
writeFileSync(
	path,
	JSON.stringify({ ...JSON.parse(readFileSync(path, 'utf8')), type: 'commonjs' }, null, 2) + '\n'
);
