import { readFile } from 'node:fs/promises';
export async function GET() {
	return new Response(await readFile('LICENSE', 'utf8'), {
		headers: {
			'Content-Type': 'text/plain; charset=utf-8',
			'Cache-Control': 'public, max-age=3600'
		}
	});
}
