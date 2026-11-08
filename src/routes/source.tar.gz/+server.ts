import { readFile } from 'node:fs/promises';
import { error } from '@sveltejs/kit';

export async function GET() {
	let archive: Uint8Array<ArrayBuffer>;
	try {
		archive = new Uint8Array(await readFile('monumla-source.tar.gz'));
	} catch {
		error(503, 'The source download is temporarily unavailable.');
	}
	return new Response(archive, {
		headers: {
			'Content-Type': 'application/gzip',
			'Content-Disposition': 'attachment; filename="monumla-source.tar.gz"',
			'Cache-Control': 'no-cache'
		}
	});
}
