export function eloChange(first: number, second: number, score: number, games: number) {
	const expected = 1 / (1 + 10 ** ((second - first) / 400));
	const k = games < 30 ? 40 : first >= 2400 ? 16 : 24;
	return Math.round(k * (score - expected));
}
