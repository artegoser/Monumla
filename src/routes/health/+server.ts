import { db } from '../../../server/db';
export async function GET() {
	try {
		await db.$queryRawUnsafe('SELECT 1');
		return new Response('ok');
	} catch {
		return new Response('Database unavailable', { status: 503 });
	}
}
