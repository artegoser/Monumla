import { randomBytes, scrypt as scryptCallback, timingSafeEqual, createHash } from 'node:crypto';
import { promisify } from 'node:util';
import { db } from './db';
import type { User } from '@prisma/client';
import { countryCodes, type PublicUser } from '../src/lib/platform/catalog';

const scrypt = promisify(scryptCallback);
export const SESSION_COOKIE = 'monumla_session';
export const SESSION_SECONDS = 60 * 60 * 24 * 30;
export const publicUserSelect = {
	id: true,
	username: true,
	country: true,
	bio: true,
	createdAt: true
} as const;
export function publicUser(user: Pick<User, keyof typeof publicUserSelect>): PublicUser {
	return {
		id: user.id,
		username: user.username,
		country: user.country,
		bio: user.bio,
		createdAt: user.createdAt.toISOString()
	};
}
export const tokenHash = (token: string) => createHash('sha256').update(token).digest('hex');

export function userMessage(
	error: unknown,
	fallback = 'The request could not be completed. Please try again.'
) {
	if (error instanceof Error && (error.name.startsWith('Prisma') || error.name === 'SqliteError')) {
		console.error('Database request failed:', error);
		return fallback;
	}
	return error instanceof Error ? error.message : typeof error === 'string' ? error : fallback;
}

export async function hashPassword(password: string) {
	const salt = randomBytes(16).toString('hex');
	const key = (await scrypt(password, salt, 64)) as Buffer;
	return `${salt}:${key.toString('hex')}`;
}

export async function verifyPassword(password: string, stored: string) {
	const [salt, hash] = stored.split(':');
	if (!salt || !hash) return false;
	const expected = Buffer.from(hash, 'hex');
	const actual = (await scrypt(password, salt, 64)) as Buffer;
	return expected.length === actual.length && timingSafeEqual(expected, actual);
}

export async function createSession(userId: string) {
	const token = randomBytes(32).toString('base64url');
	await db.session.create({
		data: {
			userId,
			tokenHash: tokenHash(token),
			expiresAt: new Date(Date.now() + SESSION_SECONDS * 1000)
		}
	});
	return token;
}

export async function authenticate(token: string | undefined | null) {
	if (!token || token.length > 128) return null;
	const session = await db.session.findUnique({
		where: { tokenHash: tokenHash(token) },
		include: { user: { select: publicUserSelect } }
	});
	if (!session || session.expiresAt.getTime() <= Date.now()) return null;
	return publicUser(session.user);
}

export function readSessionCookie(header = '') {
	return header
		.split(';')
		.map((item) => item.trim())
		.find((item) => item.startsWith(`${SESSION_COOKIE}=`))
		?.slice(SESSION_COOKIE.length + 1);
}

export function profileFields(country: string, bio: string) {
	if (country && !countryCodes.includes(country))
		throw new Error('Choose a country from the list.');
	if (bio.length > 500) throw new Error('Your biography must be 500 characters or fewer.');
	return { country, bio: bio.trim() };
}

const attempts = new Map<string, { count: number; until: number }>();
export function rateLimit(key: string, limit = 12, windowMs = 60_000) {
	const now = Date.now();
	if (attempts.size > 10_000)
		for (const [id, entry] of attempts) if (entry.until <= now) attempts.delete(id);
	let entry = attempts.get(key);
	if (!entry || entry.until <= now) {
		entry = { count: 0, until: now + windowMs };
		attempts.set(key, entry);
	}
	if (++entry.count > limit) throw new Error('Too many attempts. Please try again in a minute.');
}
