<script lang="ts">
	import { onMount } from 'svelte';
	import AnalysisPanel from '$lib/search/AnalysisPanel.svelte';
	import type { AnalysisView, BestAction, MoveAssessment } from '$lib/search/types';
	import MoveBadge from '$lib/search/MoveBadge.svelte';
	import { describeAssessment, assessmentLegend } from '$lib/search/assessment';
	import GameReview from '$lib/search/GameReview.svelte';
	import AnalysisOverlay from '$lib/search/AnalysisOverlay.svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import Bg from '$lib/board/Bg.svelte';
	import Pieces from './Pieces.svelte';
	import {
		LocalGame,
		stateScalar,
		type ChessDrawClaimsView,
		type ChoiceView,
		type GameView,
		type HistoryTurnView,
		type InteractionView,
		type GoScoring,
		type RulesetId,
		type TransitionView
	} from '$lib/wasm/runtime';

	const rulesets: Array<{ id: RulesetId; label: string; description: string }> = [
		{ id: 'chess', label: 'Chess', description: 'Standard chess with SAN, PGN and FEN.' },
		{
			id: 'checkers',
			label: 'Russian draughts',
			description: '8×8 · Backward captures, flying kings and promotion during captures.'
		},
		{
			id: 'checkers-international',
			label: 'International draughts',
			description: '10×10 · Maximum captures, flying kings and promotion after the turn.'
		},
		{
			id: 'go',
			label: 'Go',
			description: 'AGA rules with superko, scoring review, komi and handicap.'
		}
	];

	let runtime: LocalGame | null = null;
	let game: GameView | null = null;
	let previousGame: GameView | null = null;
	let interaction: InteractionView = emptyInteraction();
	let latestTransition: TransitionView | null = null;
	let animationSeq = 0;
	let history: HistoryTurnView[] = [];
	let selectedRuleset: RulesetId = 'chess';
	let currentFen = '';
	let fenDraft = '';
	let currentPgn = '';
	let pgnDraft = '';
	let chessDrawClaims: ChessDrawClaimsView | null = null;
	let orientation: 'white' | 'black' = 'white';
	let goSize = 19;
	let goScoring: GoScoring = 'territory';
	let goHandicap = 0;
	let error: string | null = null;
	let loading = true;
	let positionRevision = 0;
	let turnInProgress = false;
	let botSide: number | null = null;
	let bothBots = false;
	let controlKey = '';
	let pauseRevision = 0;
	let animationInProgress = false;
	let pendingComputerMove: BestAction | null = null;

	let mode: 'play' | 'analysis' = 'play';
	let analysis: AnalysisView | null = null;
	let lastWhiteScore: number | null = null;
	let selectedLine = 0;
	let showArrows = true;
	let showEvaluation = true;
	let showAnnotations = true;
	let showCoordinates = true;
	let fullHistory: HistoryTurnView[] = [];
	let reviewResults: Record<string, MoveAssessment> = {};
	let analyzedPositions: Record<string, AnalysisView> = {};
	$: beforeKeys = history.map((_, i) => positionId(history.slice(0, i)));
	$: reviewKeys = history.map((_, i) => positionId(history.slice(0, i + 1)));
	$: whiteSide = game?.ruleset === 'go' ? 2 : 1;
	$: bottomSide = orientation === 'white' ? whiteSide : whiteSide === 1 ? 2 : 1;
	$: topSide = bottomSide === 1 ? 2 : 1;
	$: whiteEvaluation = lastWhiteScore ?? 0;
	$: evaluationHeight = 50 + 48 * Math.tanh(whiteEvaluation / 600);
	$: historyRows = Array.from({ length: Math.ceil(fullHistory.length / 2) }, (_, i) =>
		fullHistory.slice(i * 2, i * 2 + 2)
	);

	$: currentTurn = history[history.length - 1];
	$: currentAssessment = currentTurn ? reviewResults[positionId(history)] : undefined;
	$: currentGrade = describeAssessment(currentAssessment);

	function positionId(turns = history) {
		return selectedRuleset + ':' + turns.map((t) => `${t.actor}:${t.notation}`).join('/');
	}
	function receiveAnalysis(value: AnalysisView | null) {
		analysis = value;
		if (
			value?.score != null &&
			value.best_action?.position_key === runtime?.analysisPositionKey()
		) {
			analyzedPositions = { ...analyzedPositions, [positionId()]: value };
		}
		if (value?.score != null)
			lastWhiteScore = value.score * (value.perspective === whiteSide ? 1 : -1);
	}
	function assessment(index: number, results: Record<string, MoveAssessment>) {
		const result = results[positionId(fullHistory.slice(0, index + 1))];
		if (!result?.symbol) return null;
		return {
			...result,
			symbol: result.symbol,
			bad: ['?!', '?', '??'].includes(result.symbol),
			title: `${result.label} · depth ${result.depth} · score loss ${((result.loss ?? 0) / 100).toFixed(2)}${result.best_move ? ` · best: ${result.best_move}` : ''}`
		};
	}
	function navigate(boundary: number) {
		if (!runtime) return;
		pauseRevision += 1;
		const animate = Math.abs(history.length - boundary) === 1;
		while (history.length > boundary && game?.can_undo) run(() => runtime!.undo(), animate);
		while (history.length < boundary && game?.can_redo) run(() => runtime!.redo(), animate);
	}
	function keyboard(event: KeyboardEvent) {
		if (
			event.target instanceof HTMLElement &&
			(event.target.closest('input,textarea,select') || event.target.isContentEditable)
		)
			return;
		if (event.key === 'ArrowLeft') {
			event.preventDefault();
			undo();
		}
		if (event.key === 'ArrowRight') {
			event.preventDefault();
			redo();
		}
	}
	function playerName(
		side: number,
		currentMode: string,
		computer: number | null,
		computers: boolean
	) {
		return computers && currentMode === 'play'
			? 'Monumla computer'
			: currentMode === 'analysis'
				? 'Analysis board'
				: computer === side
					? 'Monumla computer'
					: computer != null
						? 'You'
						: 'Local player';
	}

	function emptyInteraction(): InteractionView {
		return {
			generation: '0',
			selected_entity: null,
			pending_target: null,
			active_ability: null,
			choices: []
		};
	}

	$: actionChoices = interaction.choices.filter(
		(choice) =>
			choice.kind === 'select_ability' ||
			choice.kind === 'finish_turn' ||
			(choice.kind === 'select_option' && !choice.asset_key)
	);
	$: currentRuleset = rulesets.find((ruleset) => ruleset.id === selectedRuleset) ?? rulesets[0];

	function syncMetadata(resetDrafts = false) {
		if (!runtime || !game) return;
		history = runtime.history();
		if (
			!game.can_redo ||
			history.some(
				(turn, i) =>
					fullHistory[i]?.notation !== turn.notation || fullHistory[i]?.actor !== turn.actor
			)
		)
			fullHistory = history;
		if (game.ruleset === 'chess') {
			currentFen = runtime.fen();
			currentPgn = runtime.pgn();
			chessDrawClaims = runtime.chessDrawClaims();
			if (resetDrafts || !fenDraft) fenDraft = currentFen;
			if (resetDrafts || !pgnDraft) pgnDraft = currentPgn;
		} else {
			chessDrawClaims = null;
			currentFen = '';
			currentPgn = '';
			fenDraft = '';
			pgnDraft = '';
		}
		if (game.ruleset === 'go') {
			goSize = game.width;
			const scoring = stateScalar(game.status.details, 'scoring');
			if (scoring === 'territory' || scoring === 'area') goScoring = scoring;
			const handicap = stateScalar(game.status.details, 'handicap');
			if (typeof handicap === 'number') goHandicap = handicap;
		}
	}

	function applyTransition(transition: TransitionView, animate = true) {
		if (transition.committed) {
			pendingComputerMove = null;
			positionRevision += 1;
			turnInProgress = false;
		}
		turnInProgress = runtime?.turnInProgress() ?? false;
		previousGame = game;
		game = transition.game;
		interaction = transition.interaction;
		latestTransition = transition;
		if (animate && transition.changes.length > 0) {
			animationInProgress = true;
			animationSeq += 1;
		}
		// FEN, PGN, history and draw claims describe only committed state.
		// Selection/promotion drafts must not trigger expensive prospective-claim scans.
		if (transition.committed) syncMetadata();
	}

	function run(action: () => TransitionView, animate = true) {
		try {
			applyTransition(action(), animate);
			error = null;
		} catch (cause) {
			error = cause instanceof Error ? cause.message : String(cause);
		}
	}

	function choose(choiceId: string) {
		if (!runtime || (botSide != null && game?.active_players.includes(botSide))) return;
		run(() => runtime!.choose(choiceId));
	}

	function playBestAction(best: BestAction) {
		if (!runtime) return;
		if (animationInProgress) {
			pendingComputerMove = best;
			return;
		}
		run(() => runtime!.applyBestAction(best));
	}
	function animationChanged(active: boolean) {
		animationInProgress = active;
		if (!active && pendingComputerMove) {
			const best = pendingComputerMove;
			pendingComputerMove = null;
			if (runtime && best.position_key === runtime.analysisPositionKey()) playBestAction(best);
		}
	}

	function cancelSelection() {
		if (!runtime) return;
		run(() => runtime!.cancelSelection(), false);
	}

	function undo() {
		if (!runtime || !game?.can_undo) return;
		pauseRevision += 1;
		run(() => runtime!.undo());
	}

	function redo() {
		if (!runtime || !game?.can_redo) return;
		pauseRevision += 1;
		run(() => runtime!.redo());
	}

	function chessResign() {
		if (!runtime || game?.ruleset !== 'chess' || game.status.outcome) return;
		run(() => runtime!.chessResign(), false);
	}

	function chessAgreeDraw() {
		if (!runtime || game?.ruleset !== 'chess' || game.status.outcome) return;
		run(() => runtime!.chessAgreeDraw(), false);
	}

	function chessClaimDraw(kind: 'threefold_repetition' | 'fifty_move_rule') {
		if (!runtime || game?.ruleset !== 'chess' || game.status.outcome) return;
		run(() => runtime!.chessClaimDraw(kind), false);
	}

	function chessClaimDrawAfterSan(kind: 'threefold_repetition' | 'fifty_move_rule', san: string) {
		if (!runtime || game?.ruleset !== 'chess' || game.status.outcome) return;
		run(() => runtime!.chessClaimDrawAfterSan(kind, san), false);
	}

	async function installRuntime(next: LocalGame) {
		const old = runtime;
		runtime = next;
		positionRevision += 1;
		turnInProgress = false;
		selectedRuleset = next.ruleset();
		fullHistory = [];
		reviewResults = {};
		analyzedPositions = {};
		analysis = null;
		lastWhiteScore = null;
		previousGame = null;
		latestTransition = null;
		animationSeq += 1;
		game = next.view();
		interaction = next.interaction();
		history = next.history();
		error = null;
		loading = false;
		syncMetadata(true);
		old?.dispose();
	}

	async function switchRuleset(ruleset: RulesetId) {
		if (loading || ruleset === selectedRuleset) return;
		try {
			loading = true;
			await installRuntime(await LocalGame.create(ruleset));
		} catch (cause) {
			loading = false;
			error = cause instanceof Error ? cause.message : String(cause);
		}
	}

	async function resetGame() {
		try {
			loading = true;
			const next =
				selectedRuleset === 'go'
					? await LocalGame.createGo(goSize, goScoring, goHandicap)
					: await LocalGame.create(selectedRuleset);
			await installRuntime(next);
		} catch (cause) {
			loading = false;
			error = cause instanceof Error ? cause.message : String(cause);
		}
	}

	function handleGoSizeChange() {
		if (goSize !== 19 && goHandicap > 1) goHandicap = 0;
	}

	async function newGoGame() {
		try {
			loading = true;
			if (goSize !== 19 && goHandicap > 1) goHandicap = 0;
			await installRuntime(await LocalGame.createGo(goSize, goScoring, goHandicap));
		} catch (cause) {
			loading = false;
			error = cause instanceof Error ? cause.message : String(cause);
		}
	}

	async function loadFen() {
		try {
			loading = true;
			await installRuntime(await LocalGame.fromFen(fenDraft.trim()));
		} catch (cause) {
			loading = false;
			error = cause instanceof Error ? cause.message : String(cause);
		}
	}

	async function loadPgn() {
		try {
			loading = true;
			await installRuntime(await LocalGame.fromPgn(pgnDraft.trim()));
		} catch (cause) {
			loading = false;
			error = cause instanceof Error ? cause.message : String(cause);
		}
	}

	async function copyText(value: string) {
		if (!value || typeof navigator === 'undefined') return;
		try {
			await navigator.clipboard.writeText(value);
		} catch {
			// The source field remains selectable when clipboard permission is unavailable.
		}
	}

	function actionLabel(choice: ChoiceView) {
		if (choice.label) return choice.label;
		if (choice.kind === 'finish_turn') return 'Finish turn';
		if (choice.kind === 'select_ability') return `Ability ${choice.ability ?? ''}`.trim();
		return choice.option_key ?? 'Choose';
	}

	function goDetail(key: string, current: GameView | null) {
		return current?.ruleset === 'go' ? stateScalar(current.status.details, key) : null;
	}

	function halfPointText(value: string | number | boolean | null) {
		if (typeof value !== 'number') return '—';
		const sign = value < 0 ? '-' : '';
		const absolute = Math.abs(value);
		return absolute % 2 === 0 ? `${sign}${absolute / 2}` : `${sign}${Math.floor(absolute / 2)}.5`;
	}

	onMount(() => {
		let mounted = true;
		LocalGame.create('chess')
			.then((loaded) => {
				if (!mounted) {
					loaded.dispose();
					return;
				}
				return installRuntime(loaded);
			})
			.catch((cause) => {
				loading = false;
				error = cause instanceof Error ? cause.message : String(cause);
			});

		return () => {
			mounted = false;
			runtime?.dispose();
			runtime = null;
		};
	});
</script>

<svelte:window on:keydown={keyboard} />
<header class="app-header">
	<a href="/" class="wordmark">monumla<span class="brand-dot">.</span></a>
	<nav class="mode-tabs" aria-label="Board mode">
		<button
			type="button"
			class:active={mode === 'play'}
			aria-pressed={mode === 'play'}
			on:click={() => (mode = 'play')}>Play</button
		>
		<button
			type="button"
			class:active={mode === 'analysis'}
			aria-pressed={mode === 'analysis'}
			on:click={() => (mode = 'analysis')}>Analysis</button
		>
	</nav>
	<select
		aria-label="Game variant"
		value={selectedRuleset}
		disabled={loading}
		on:change={(e) => switchRuleset(e.currentTarget.value as RulesetId)}
	>
		{#each rulesets as ruleset}<option value={ruleset.id}>{ruleset.label}</option>{/each}
	</select>
</header>
{#if game && runtime}
	<main class="game-layout" class:analysis-mode={mode === 'analysis'}>
		<div class="board-column">
			<div class="player-strip" class:active-player={game.active_players.includes(topSide)}>
				<span class="player-avatar"><Icon name={botSide === topSide ? 'bot' : 'user'} /></span>
				<div>
					<strong>{playerName(topSide, mode, botSide, bothBots)}</strong><span
						>{topSide === whiteSide ? 'White' : 'Black'}</span
					>
				</div>
				<span class="turn-indicator"
					>{game.active_players.includes(topSide) && !game.status.outcome ? 'To move' : ''}</span
				>
			</div>
			<div class="board-with-evaluation">
				{#if mode === 'analysis' && showEvaluation}
					<div
						class="evaluation-bar"
						class:pending={analysis == null && lastWhiteScore != null}
						class:inverted={orientation === 'black'}
						aria-label="Evaluation bar"
						title="Generic heuristic evaluation, White perspective"
					>
						<div style:height={`${evaluationHeight}%`}></div>
					</div>
				{/if}
				<div
					class="board-frame"
					class:orientation-black={orientation === 'black'}
					class:go-frame={game.board_style === 'go'}
				>
					<div class="board">
						<Bg
							width={game.width}
							height={game.height}
							{orientation}
							boardStyle={game.board_style}
							showCoordinates={mode === 'play' || showCoordinates}
						/>
						<Pieces
							onanimationchange={animationChanged}
							{game}
							interaction={botSide != null && game.active_players.includes(botSide)
								? emptyInteraction()
								: interaction}
							{previousGame}
							transition={latestTransition}
							{animationSeq}
							{orientation}
							onchoice={choose}
							oncancel={cancelSelection}
						/>
						{#if mode === 'analysis' && showArrows && !turnInProgress}
							<AnalysisOverlay
								lines={analysis?.lines ?? []}
								selected={selectedLine}
								width={game.width}
								height={game.height}
								{orientation}
							/>
						{/if}
						{#if mode === 'analysis' && showAnnotations && game.last_move && history.length}
							{@const note = assessment(history.length - 1, reviewResults)}
							{#if note}<span
									class="board-assessment"
									class:bad={note.bad}
									title={note.title}
									style:left={`${(((orientation === 'white' ? game.last_move.to.x : game.width - 1 - game.last_move.to.x) + 0.8) * 100) / game.width}%`}
									style:top={`${(((orientation === 'white' ? game.height - 1 - game.last_move.to.y : game.last_move.to.y) + 0.1) * 100) / game.height}%`}
									><MoveBadge assessment={note} title={note.title} /></span
								>{/if}
						{/if}
					</div>
				</div>
			</div>
			<div class="player-strip" class:active-player={game.active_players.includes(bottomSide)}>
				<span class="player-avatar"><Icon name={botSide === bottomSide ? 'bot' : 'user'} /></span>
				<div>
					<strong>{playerName(bottomSide, mode, botSide, bothBots)}</strong><span
						>{bottomSide === whiteSide ? 'White' : 'Black'}</span
					>
				</div>
				<span class="turn-indicator"
					>{game.active_players.includes(bottomSide) && !game.status.outcome ? 'To move' : ''}</span
				>
			</div>
			<div class="board-footer">
				<span class="board-status" class:terminal={game.status.outcome != null}
					>{game.status.text}</span
				><button
					class="icon-button"
					type="button"
					aria-label="Flip board"
					title="Flip board"
					on:click={() => (orientation = orientation === 'white' ? 'black' : 'white')}
					><Icon name="flip" /></button
				>
			</div>
			{#if actionChoices.length > 0}<div class="action-bar" aria-label="Available actions">
					{#each actionChoices as choice}<button
							type="button"
							disabled={botSide != null && game.active_players.includes(botSide)}
							on:click={() => choose(choice.id)}>{actionLabel(choice)}</button
						>{/each}
				</div>{/if}
		</div>
		<aside class="game-panel">
			<div class="session-heading">
				<div>
					<h1>{game.title}</h1>
					<span>{mode === 'analysis' ? 'Explore every possibility' : 'A board. A good game.'}</span>
				</div>
				<button
					class="icon-button"
					type="button"
					on:click={resetGame}
					aria-label="New game"
					title="New game"><Icon name="reset" /></button
				>
			</div>
			<AnalysisPanel
				{runtime}
				{game}
				{mode}
				revision={positionRevision}
				{pauseRevision}
				{turnInProgress}
				bind:selectedLine
				bind:showArrows
				bind:showEvaluation
				bind:showAnnotations
				bind:showCoordinates
				onplay={playBestAction}
				onanalysis={receiveAnalysis}
				oncontrolchange={(side, human, white, both, paused) => {
					botSide = side;
					if (paused || side == null) pendingComputerMove = null;
					bothBots = both;
					const key = `${side != null}:${human}:${white}:${both}`;
					if (key !== controlKey && side != null)
						orientation = both || human === white ? 'white' : 'black';
					controlKey = key;
				}}
			/>
			<section class="history-panel" aria-label="Move history">
				<div class="panel-heading-row">
					<h2>Moves</h2>
					<span class="subtle">{history.length} plies</span>
				</div>
				{#if mode === 'analysis' && showAnnotations}
					<div
						class="move-insight"
						aria-label="Current move assessment"
						style:--quality-color={currentGrade.color}
						data-quality={currentGrade.tone}
					>
						{#if currentAssessment}<MoveBadge assessment={currentAssessment} />{:else}<span
								class="assessment-pending">…</span
							>{/if}
						<div>
							<strong
								>{currentTurn
									? `${currentTurn.notation} · ${currentGrade.name}`
									: 'Explore a continuation'}</strong
							><small
								>{currentAssessment
									? `Engine estimate · depth ${currentAssessment.depth}${currentAssessment.best_move && currentAssessment.loss && currentAssessment.loss >= 40 ? ` · Better: ${currentAssessment.best_move}` : ''}`
									: currentTurn
										? 'Reviewing this move…'
										: 'Move assessments appear here'}</small
							>
						</div>
					</div>
					<div class="review-slot">
						{#if !turnInProgress}<GameReview
								analyses={analyzedPositions}
								{beforeKeys}
								{runtime}
								{history}
								revision={positionRevision}
								keys={reviewKeys}
								results={reviewResults}
								onassessment={(key, value) => (reviewResults = { ...reviewResults, [key]: value })}
							/>{/if}
					</div>
					<details class="assessment-key">
						<summary>Move symbols</summary>
						<div>
							{#each assessmentLegend as grade}<span style:--quality-color={grade.color}
									><b>{grade.symbol}</b>{grade.name}</span
								>{/each}
						</div>
					</details>
				{/if}
				<div class="move-list">
					{#if !fullHistory.length}<p class="history-empty">
							{mode === 'analysis'
								? 'Make a move to explore a continuation.'
								: 'Your next game starts here.'}
						</p>{/if}
					{#each historyRows as row, rowIndex}<div class="move-row">
							<span class="move-number">{row[0].turn_number}.</span
							>{#each row as turn, column}<button
									type="button"
									class:current={history.length === rowIndex * 2 + column + 1}
									class:future={history.length < rowIndex * 2 + column + 1}
									on:click={() => navigate(rowIndex * 2 + column + 1)}
									aria-label={`Go to move ${rowIndex * 2 + column + 1}: ${turn.notation}`}
									><span>{turn.notation}</span
									>{#if mode === 'analysis' && showAnnotations}{@const note = assessment(
											rowIndex * 2 + column,
											reviewResults
										)}{#if note}<span
												class="move-assessment"
												class:bad={note.bad}
												title={note.title}><MoveBadge assessment={note} title={note.title} /></span
											>{/if}{/if}</button
								>{/each}
						</div>{/each}
				</div>
				<div class="history-controls" aria-label="Move navigation">
					<button
						class="icon-button"
						type="button"
						aria-label="First move"
						on:click={() => navigate(0)}
						disabled={!game.can_undo}><Icon name="first" /></button
					><button
						class="icon-button"
						type="button"
						aria-label="Undo"
						on:click={undo}
						disabled={!game.can_undo}><Icon name="back" /></button
					><button
						class="icon-button"
						type="button"
						aria-label="Redo"
						on:click={redo}
						disabled={!game.can_redo}><Icon name="next" /></button
					><button
						class="icon-button"
						type="button"
						aria-label="Latest move"
						on:click={() => navigate(fullHistory.length)}
						disabled={!game.can_redo}><Icon name="last" /></button
					>
				</div>
			</section>
			{#if game.ruleset === 'go'}
				<details class="panel-details">
					<summary>Go rules & scoring</summary>
					<section>
						<h2>AGA game</h2>
						<div class="go-settings">
							<label>
								Board
								<select aria-label="Board" bind:value={goSize} on:change={handleGoSizeChange}>
									<option value={9}>9×9</option>
									<option value={13}>13×13</option>
									<option value={19}>19×19</option>
								</select>
							</label>
							<label>
								Counting
								<select bind:value={goScoring}>
									<option value="territory">Territory</option>
									<option value="area">Area</option>
								</select>
							</label>
							<label>
								Handicap
								<select bind:value={goHandicap}>
									{#each { length: 10 } as _, handicap}
										<option value={handicap} disabled={goSize !== 19 && handicap > 1}
											>{handicap === 0 ? 'Even' : handicap}</option
										>
									{/each}
								</select>
							</label>
							<button type="button" on:click={newGoGame} disabled={loading}>New game</button>
						</div>
						<div class="go-score-grid">
							<span>Rules</span><strong>AGA</strong>
							<span>Komi</span><strong>{halfPointText(goDetail('komi_half', game))}</strong>
							<span>Black prisoners</span><strong>{goDetail('black_prisoners', game) ?? 0}</strong>
							<span>White prisoners</span><strong>{goDetail('white_prisoners', game) ?? 0}</strong>
							<span>Black pass stones</span><strong
								>{goDetail('black_pass_stones', game) ?? 0}</strong
							>
							<span>White pass stones</span><strong
								>{goDetail('white_pass_stones', game) ?? 0}</strong
							>
						</div>
						{#if goDetail('phase', game) === 'review'}
							<p class="panel-muted">
								Click stone groups to mark their status, then choose Done reviewing. If the
								assessments differ, resume play resolves the dispute under AGA rules.
							</p>
						{/if}
						{#if game.status.outcome?.key === 'go.score'}
							<div class="go-final-score">
								<strong>Final score</strong>
								<span>Black {halfPointText(goDetail('black_score_half', game))}</span>
								<span>White {halfPointText(goDetail('white_score_half', game))}</span>
								<span
									>Territory {goDetail('black_territory', game) ?? 0}–{goDetail(
										'white_territory',
										game
									) ?? 0} · neutral
									{goDetail('neutral_points', game) ?? 0}</span
								>
							</div>
						{/if}
					</section>
				</details>
			{/if}
			{#if game.ruleset === 'chess'}
				<details class="panel-details">
					<summary>Game actions</summary>
					<section>
						{#if game.status.outcome}
							<p class="panel-muted">
								The game is finished. Undo the terminal action to continue locally.
							</p>
						{:else}
							<div class="action-bar" aria-label="Chess game actions">
								<button type="button" on:click={chessResign}>Resign</button>
								{#if chessDrawClaims?.can_agree_draw}
									<button type="button" on:click={chessAgreeDraw}>Draw by agreement</button>
								{/if}
								{#if chessDrawClaims?.current_threefold_repetition}
									<button type="button" on:click={() => chessClaimDraw('threefold_repetition')}
										>Claim threefold</button
									>
								{/if}
								{#if chessDrawClaims?.current_fifty_move_rule}
									<button type="button" on:click={() => chessClaimDraw('fifty_move_rule')}
										>Claim 50-move draw</button
									>
								{/if}
							</div>
							{#if chessDrawClaims && chessDrawClaims.by_move.length > 0}
								<div class="unit-list">
									{#each chessDrawClaims.by_move as claim}
										<div class="unit-row">
											<span
												>Claim {claim.kind === 'threefold_repetition' ? 'threefold' : '50-move'} by declaring
												<strong>{claim.san}</strong></span
											>
											<button
												type="button"
												on:click={() => chessClaimDrawAfterSan(claim.kind, claim.san)}>Claim</button
											>
										</div>
									{/each}
								</div>
							{/if}
						{/if}
					</section>
				</details>

				<details class="panel-details">
					<summary>Position · FEN</summary>
					<section>
						<div class="fen-current">
							<input readonly value={currentFen} aria-label="Current FEN" />
							<button type="button" on:click={() => copyText(currentFen)}>Copy</button>
						</div>
						<form on:submit|preventDefault={loadFen} class="fen-loader">
							<input bind:value={fenDraft} spellcheck="false" aria-label="Load FEN" />
							<button type="submit">Load FEN</button>
						</form>
					</section>
				</details>

				<details class="panel-details">
					<summary>Game · PGN</summary>
					<section class="pgn-panel">
						<div class="panel-heading-row">
							<h2>PGN</h2>
							<button type="button" on:click={() => copyText(currentPgn)}>Copy</button>
						</div>
						<textarea readonly value={currentPgn} rows="9" aria-label="Current PGN"></textarea>
						<form on:submit|preventDefault={loadPgn} class="pgn-loader">
							<textarea bind:value={pgnDraft} rows="6" spellcheck="false" aria-label="Load PGN"
							></textarea>
							<button type="submit">Load PGN</button>
						</form>
					</section>
				</details>
			{/if}
			<div class="rules-note">{currentRuleset.description}</div>
		</aside>
	</main>
{:else}<p class="runtime-loading" role="status">
		{loading ? 'Setting up your board…' : 'Board unavailable.'}
	</p>{/if}
{#if error}<p class="runtime-error" role="alert">{error}</p>{/if}
