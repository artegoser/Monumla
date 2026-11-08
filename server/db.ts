import 'dotenv/config';
import { mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { PrismaClient } from '@prisma/client';
import { PrismaBetterSqlite3 } from '@prisma/adapter-better-sqlite3';

const shared = globalThis as typeof globalThis & { monumlaDb?: PrismaClient };
const databaseUrl = process.env.DATABASE_URL ?? `file:${resolve('data/monumla.db')}`;
if (!databaseUrl.startsWith('file:')) throw new Error('DATABASE_URL must be a SQLite file URL.');
mkdirSync(dirname(resolve(databaseUrl.slice(5))), { recursive: true });
export const db = (shared.monumlaDb ??= new PrismaClient({
	adapter: new PrismaBetterSqlite3({ url: databaseUrl, timeout: 5000 })
}));

export async function initializeDatabase() {
	await db.$queryRawUnsafe('PRAGMA journal_mode = WAL');
	await db.$queryRawUnsafe('PRAGMA busy_timeout = 5000');
}
