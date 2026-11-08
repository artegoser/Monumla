import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';

// An explicit source allowlist excludes credentials, databases, and local caches.
const paths = [
	'src',
	'server',
	'crates',
	'prisma',
	'scripts',
	'tests',
	'static',
	'docs',
	'package.json',
	'pnpm-lock.yaml',
	'pnpm-workspace.yaml',
	'Cargo.toml',
	'Cargo.lock',
	'rust-toolchain.toml',
	'prisma.config.ts',
	'svelte.config.js',
	'vite.config.ts',
	'tsconfig.json',
	'tsconfig.server.json',
	'tailwind.config.ts',
	'postcss.config.js',
	'eslint.config.js',
	'playwright.config.ts',
	'Dockerfile',
	'compose.yaml',
	'.dockerignore',
	'.npmrc',
	'.prettierrc',
	'.prettierignore',
	'.env.example',
	'README.md',
	'LICENSE',
	'PRODUCT.md',
	'DESIGN.md'
].filter(existsSync);
const result = spawnSync(
	'tar',
	[
		'--exclude=*.db',
		'--exclude=*.db-wal',
		'--exclude=*.db-shm',
		'--exclude=*/.env',
		'--exclude=*/.env.*',
		'--exclude=__pycache__',
		'--exclude=server/wasm',
		'--exclude=src/lib/wasm/pkg',
		'--exclude=static/source.tar.gz',
		'-czf',
		'monumla-source.tar.gz',
		...paths
	],
	{ stdio: 'inherit' }
);
if (result.status !== 0) process.exit(result.status ?? 1);
