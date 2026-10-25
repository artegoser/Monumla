/** Required on the document AND worker entry responses for shared cancellation. */
export const isolationHeaders = {
	'Cross-Origin-Opener-Policy': 'same-origin',
	'Cross-Origin-Embedder-Policy': 'require-corp'
};

/** @param {import('node:http').ServerResponse} response */
export function applyIsolationHeaders(response) {
	for (const [name, value] of Object.entries(isolationHeaders)) response.setHeader(name, value);
}
