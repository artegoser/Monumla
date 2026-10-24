export interface SearchSettings {
	forcedMove?: boolean;
	maxDepth: number;
	maxNodes: number;
	timeMs: number;
	lines: number;
}

export interface BestAction {
	position_key: string;
	action: unknown;
}

export interface AnalysisLine {
	score: number;
	moves: string[];
	path: Array<{ x: number; y: number }>;
}

export interface LiveAnalysis {
	depth: number;
	root_done: number;
	root_total: number;
	line_index: number;
	current_move: string | null;
	bound: 'exact' | 'lower' | 'upper' | 'estimate';
	line: AnalysisLine | null;
}
export interface AnalysisView {
	position_key?: string;
	iteration?: LiveAnalysis | null;
	samples: number;
	lines: AnalysisLine[];
	score: number | null;
	perspective: number;
	depth: number;
	nodes: number;
	elapsed_ms: number;
	nodes_per_second: number;
	principal_variation: string[];
	best_action: BestAction | null;
	stop_reason: 'depth_limit' | 'node_limit' | 'time_limit' | 'terminal' | 'forced_move';
	transposition_hits: number;
	quiescence_nodes: number;
	cutoffs: number;
}

export interface MoveAssessment {
	samples: number;
	depth: number;
	best_score: number | null;
	played_score: number | null;
	loss: number | null;
	symbol: string | null;
	label: string;
	best_move: string | null;
}

export interface SearchRequest {
	cancel?: boolean;
	ply?: number;
	id: number;
	snapshot: unknown;
	settings: SearchSettings;
}

export type SearchMessage =
	| { id: number; type: 'progress' | 'result'; analysis: AnalysisView }
	| { id: number; type: 'review'; review: MoveAssessment }
	| { id: number; type: 'error'; message: string };
