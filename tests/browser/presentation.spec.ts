import { expect, test } from '@playwright/test';
import { displayLines, displayScore } from '../../src/lib/search/presentation';
import type { AnalysisView } from '../../src/lib/search/types';

test('unfinished root bounds never replace the ranked moves, arrows or large evaluation', () => {
	const complete: AnalysisView = {
		depth: 2,
		score: 20,
		lines: [{ score: 20, moves: ['A1'], path: [{ x: 0, y: 0 }] }],
		iteration: null,
		samples: 0,
		perspective: 1,
		nodes: 100,
		elapsed_ms: 10,
		nodes_per_second: 10000,
		principal_variation: ['A1'],
		best_action: null,
		stop_reason: 'depth_limit',
		transposition_hits: 0,
		quiescence_nodes: 0,
		cutoffs: 0
	};
	for (const score of [-500, 800, 0]) {
		const live: AnalysisView = {
			...complete,
			iteration: {
				depth: 3,
				root_done: 1,
				root_total: 20,
				line_index: 1,
				current_move: 'B2',
				bound: 'lower',
				line: { score, moves: ['B2'], path: [{ x: 1, y: 1 }] }
			}
		};
		expect(displayLines(live)).toEqual(displayLines(complete));
		expect(displayScore(live)).toBe(20);
	}
});
