export type RulesetId = 'chess' | 'checkers' | 'checkers-international' | 'go';
export type GoScoring = 'territory' | 'area';

export interface PositionView {
	x: number;
	y: number;
}

export type StateValueView =
	| { type: 'bool'; value: boolean }
	| { type: 'i64'; value: number }
	| { type: 'u64'; value: number }
	| { type: 'f64'; value: number }
	| { type: 'string'; value: string }
	| { type: 'list'; value: StateValueView[] }
	| { type: 'map'; value: StateMapView };

export type StateMapView = Record<string, StateValueView>;

export function stateScalar(
	data: StateMapView | null | undefined,
	key: string
): string | number | boolean | null {
	const value = data?.[key];
	if (!value) return null;
	switch (value.type) {
		case 'bool':
		case 'i64':
		case 'u64':
		case 'f64':
		case 'string':
			return value.value;
		default:
			return null;
	}
}

export interface EntityView {
	id: number;
	entity_type: number;
	owner: number;
	controller: number;
	position: PositionView;
	asset_key: string;
	label: string | null;
	presentation_data: StateMapView;
	state: StateMapView;
}

export interface OutcomeView {
	key: string;
	winners: number[];
	losers: number[];
	winning_teams: number[];
	losing_teams: number[];
	data: StateMapView;
}

export interface StatusView {
	text: string;
	outcome: OutcomeView | null;
	checked_position: PositionView | null;
	details: StateMapView;
}

export interface MoveEndpointsView {
	from: PositionView;
	to: PositionView;
}

export interface GameView {
	ruleset: RulesetId;
	title: string;
	board_style: 'checkerboard' | 'go';
	width: number;
	height: number;
	entities: EntityView[];
	last_move: MoveEndpointsView | null;
	active_players: number[];
	status: StatusView;
	can_undo: boolean;
	can_redo: boolean;
}

export interface ChoiceView {
	id: string;
	kind: 'select_entity' | 'select_position' | 'select_ability' | 'select_option' | 'finish_turn';
	entity: number | null;
	position: PositionView | null;
	ability: number | null;
	option_key: string | null;
	label: string | null;
	actor: number | null;
	capture: number | null;
	move_kind: string | null;
	target_position: PositionView | null;
	option_entity_type: number | null;
	asset_key: string | null;
	data: StateMapView;
}

export interface InteractionView {
	generation: string;
	selected_entity: number | null;
	pending_target: PositionView | null;
	active_ability: number | null;
	choices: ChoiceView[];
}

export interface ChangeView {
	type: string;
	[key: string]: unknown;
}

export interface PresentationView {
	kind: string;
	data: StateMapView;
}

export interface TransitionView {
	committed: boolean;
	game: GameView;
	interaction: InteractionView;
	changes: ChangeView[];
	presentation: PresentationView[];
}

export interface ChessDrawClaimMoveView {
	kind: 'threefold_repetition' | 'fifty_move_rule';
	san: string;
}

export interface ChessDrawClaimsView {
	can_agree_draw: boolean;
	current_threefold_repetition: boolean;
	current_fifty_move_rule: boolean;
	by_move: ChessDrawClaimMoveView[];
}

export interface HistoryTurnView {
	index: number;
	actor: number;
	turn_number: number;
	actor_label: string;
	notation: string;
	actions: Array<{ kind: string; data: StateMapView }>;
}

interface WasmHandle {
	ruleset(): string;
	view(): GameView;
	interaction(): InteractionView;
	choose(choiceId: string): TransitionView;
	cancelSelection(): TransitionView;
	undo(): TransitionView;
	redo(): TransitionView;
	fen(): string;
	pgn(): string;
	chessDrawClaims(): ChessDrawClaimsView;
	chessResign(): TransitionView;
	chessAgreeDraw(): TransitionView;
	chessClaimDraw(kind: string): TransitionView;
	chessClaimDrawAfterSan(kind: string, san: string): TransitionView;
	history(): HistoryTurnView[];
	canUndo(): boolean;
	canRedo(): boolean;
	free(): void;
}

interface WasmModule {
	default(): Promise<unknown>;
	new_game(ruleset: string): WasmHandle;
	new_go(size: number, scoring: string, handicap: number): WasmHandle;
	from_fen(fen: string): WasmHandle;
	from_pgn(pgn: string): WasmHandle;
}

let modulePromise: Promise<WasmModule> | null = null;

async function loadModule(): Promise<WasmModule> {
	modulePromise ??= import('./pkg/monumla.js').then(async (module) => {
		const wasm = module as unknown as WasmModule;
		await wasm.default();
		return wasm;
	});
	return modulePromise;
}

export class LocalGame {
	private constructor(private readonly handle: WasmHandle) {}

	static async create(ruleset: RulesetId): Promise<LocalGame> {
		const wasm = await loadModule();
		return new LocalGame(wasm.new_game(ruleset));
	}

	static async createGo(
		size: number,
		scoring: GoScoring = 'territory',
		handicap = 0
	): Promise<LocalGame> {
		const wasm = await loadModule();
		return new LocalGame(wasm.new_go(size, scoring, handicap));
	}

	static async fromFen(fen: string): Promise<LocalGame> {
		const wasm = await loadModule();
		return new LocalGame(wasm.from_fen(fen));
	}

	static async fromPgn(pgn: string): Promise<LocalGame> {
		const wasm = await loadModule();
		return new LocalGame(wasm.from_pgn(pgn));
	}

	ruleset(): RulesetId {
		return this.handle.ruleset() as RulesetId;
	}

	view(): GameView {
		return this.handle.view();
	}

	interaction(): InteractionView {
		return this.handle.interaction();
	}

	choose(choiceId: string): TransitionView {
		return this.handle.choose(choiceId);
	}

	cancelSelection(): TransitionView {
		return this.handle.cancelSelection();
	}

	undo(): TransitionView {
		return this.handle.undo();
	}

	redo(): TransitionView {
		return this.handle.redo();
	}

	fen(): string {
		return this.handle.fen();
	}

	pgn(): string {
		return this.handle.pgn();
	}

	chessDrawClaims(): ChessDrawClaimsView {
		return this.handle.chessDrawClaims();
	}

	chessResign(): TransitionView {
		return this.handle.chessResign();
	}

	chessAgreeDraw(): TransitionView {
		return this.handle.chessAgreeDraw();
	}

	chessClaimDraw(kind: ChessDrawClaimMoveView['kind']): TransitionView {
		return this.handle.chessClaimDraw(kind);
	}

	chessClaimDrawAfterSan(kind: ChessDrawClaimMoveView['kind'], san: string): TransitionView {
		return this.handle.chessClaimDrawAfterSan(kind, san);
	}

	history(): HistoryTurnView[] {
		return this.handle.history();
	}

	canUndo(): boolean {
		return this.handle.canUndo();
	}

	canRedo(): boolean {
		return this.handle.canRedo();
	}

	dispose(): void {
		this.handle.free();
	}
}
