import type { AnalysisLine, AnalysisView } from './types';
export type DisplayLine = AnalysisLine & { depth: number };
/** Root prefixes are bounds, not a ranking. Keep them out of the three arrows. */
export function displayLines(analysis: AnalysisView | null, limit = 3): DisplayLine[] {
	return analysis?.lines.slice(0, limit).map((line) => ({ ...line, depth: analysis.depth })) ?? [];
}
/** The large evaluation belongs to the same completed result as the ranked lines. */
export function displayScore(analysis: AnalysisView | null) {
	return analysis?.score ?? null;
}
