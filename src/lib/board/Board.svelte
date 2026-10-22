<script lang="ts">
	import { onMount } from 'svelte';
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
		{ id: 'checkers', label: 'Checkers', description: 'Mandatory captures, chains and kinging.' },
		{ id: 'go', label: 'Go', description: 'AGA rules with superko, scoring review, komi and handicap.' },
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
		previousGame = game;
		game = transition.game;
		interaction = transition.interaction;
		latestTransition = transition;
		if (animate && transition.changes.length > 0) animationSeq += 1;
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
		if (!runtime) return;
		run(() => runtime!.choose(choiceId));
	}

	function cancelSelection() {
		if (!runtime) return;
		run(() => runtime!.cancelSelection(), false);
	}

	function undo() {
		if (!runtime || !game?.can_undo) return;
		run(() => runtime!.undo());
	}

	function redo() {
		if (!runtime || !game?.can_redo) return;
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

	function chessClaimDrawAfterSan(
		kind: 'threefold_repetition' | 'fifty_move_rule',
		san: string
	) {
		if (!runtime || game?.ruleset !== 'chess' || game.status.outcome) return;
		run(() => runtime!.chessClaimDrawAfterSan(kind, san), false);
	}

	async function installRuntime(next: LocalGame) {
		const old = runtime;
		runtime = next;
		selectedRuleset = next.ruleset();
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


	function goDetail(key: string) {
		return game?.ruleset === 'go' ? stateScalar(game.status.details, key) : null;
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

<div class="ruleset-tabs" aria-label="Monumla rulesets">
	{#each rulesets as ruleset}
		<button
			type="button"
			class:active={selectedRuleset === ruleset.id}
			disabled={loading}
			on:click={() => switchRuleset(ruleset.id)}
		>{ruleset.label}</button>
	{/each}
</div>

{#if game}
	<div class="game-layout">
		<div class="board-column">
			<div class="ruleset-heading">
				<div>
					<h1>{game.title}</h1>
					<p>{currentRuleset.description}</p>
				</div>
				<span class="ruleset-id">{game.ruleset}</span>
			</div>

			<div class="board-frame" class:orientation-black={orientation === 'black'} class:go-frame={game.board_style === 'go'}>
				<div class="board">
					<Bg
						width={game.width}
						height={game.height}
						{orientation}
						boardStyle={game.board_style}
						showCoordinates={true}
					/>
					<Pieces
						{game}
						{interaction}
						{previousGame}
						transition={latestTransition}
						{animationSeq}
						{orientation}
						onchoice={choose}
						oncancel={cancelSelection}
					/>
				</div>
			</div>

			<div class="board-status" class:terminal={game.status.outcome != null}>
				<span>{game.status.text}</span>
				<span class="board-status-meta">
					{#if game.active_players.length > 0}active {game.active_players.join(', ')}{/if}
				</span>
			</div>

			{#if actionChoices.length > 0}
				<div class="action-bar" aria-label="Available actions">
					{#each actionChoices as choice}
						<button type="button" on:click={() => choose(choice.id)}>{actionLabel(choice)}</button>
					{/each}
				</div>
			{/if}

			<div class="board-toolbar" aria-label="Board controls">
				<button type="button" on:click={undo} disabled={!game.can_undo}>Undo</button>
				<button type="button" on:click={redo} disabled={!game.can_redo}>Redo</button>
				<button type="button" on:click={resetGame}>Reset</button>
				<button type="button" on:click={() => (orientation = orientation === 'white' ? 'black' : 'white')}>Flip board</button>
			</div>
		</div>

		<aside class="game-panel">


			{#if game.ruleset === 'go'}
				<section>
					<h2>AGA game</h2>
					<div class="go-settings">
						<label>
							Board
							<select bind:value={goSize} on:change={handleGoSizeChange}>
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
									<option value={handicap} disabled={goSize !== 19 && handicap > 1}>{handicap === 0 ? 'Even' : handicap}</option>
								{/each}
							</select>
						</label>
						<button type="button" on:click={newGoGame} disabled={loading}>New game</button>
					</div>
					<div class="go-score-grid">
						<span>Rules</span><strong>AGA</strong>
						<span>Komi</span><strong>{halfPointText(goDetail('komi_half'))}</strong>
						<span>Black prisoners</span><strong>{goDetail('black_prisoners') ?? 0}</strong>
						<span>White prisoners</span><strong>{goDetail('white_prisoners') ?? 0}</strong>
						<span>Black pass stones</span><strong>{goDetail('black_pass_stones') ?? 0}</strong>
						<span>White pass stones</span><strong>{goDetail('white_pass_stones') ?? 0}</strong>
					</div>
					{#if goDetail('phase') === 'review'}
						<p class="panel-muted">Click stone groups to mark their status, then choose Done reviewing. If the assessments differ, resume play resolves the dispute under AGA rules.</p>
					{/if}
					{#if game.status.outcome?.key === 'go.score'}
						<div class="go-final-score">
							<strong>Final score</strong>
							<span>Black {halfPointText(goDetail('black_score_half'))}</span>
							<span>White {halfPointText(goDetail('white_score_half'))}</span>
							<span>Territory {goDetail('black_territory') ?? 0}–{goDetail('white_territory') ?? 0} · neutral {goDetail('neutral_points') ?? 0}</span>
						</div>
					{/if}
				</section>
			{/if}

			<section class="history-panel">
				<h2>History</h2>
				{#if history.length === 0}
					<p class="panel-muted">No turns yet.</p>
				{:else}
					<ol>
						{#each history as turn}
							<li><span>{turn.turn_number}. {turn.actor_label}</span> {turn.notation}</li>
						{/each}
					</ol>
				{/if}
			</section>

			{#if game.ruleset === 'chess'}
				<section>
					<h2>Game actions</h2>
					{#if game.status.outcome}
						<p class="panel-muted">The game is finished. Undo the terminal action to continue locally.</p>
					{:else}
						<div class="action-bar" aria-label="Chess game actions">
							<button type="button" on:click={chessResign}>Resign</button>
							{#if chessDrawClaims?.can_agree_draw}
								<button type="button" on:click={chessAgreeDraw}>Draw by agreement</button>
							{/if}
							{#if chessDrawClaims?.current_threefold_repetition}
								<button type="button" on:click={() => chessClaimDraw('threefold_repetition')}>Claim threefold</button>
							{/if}
							{#if chessDrawClaims?.current_fifty_move_rule}
								<button type="button" on:click={() => chessClaimDraw('fifty_move_rule')}>Claim 50-move draw</button>
							{/if}
						</div>
						{#if chessDrawClaims && chessDrawClaims.by_move.length > 0}
							<div class="unit-list">
								{#each chessDrawClaims.by_move as claim}
									<div class="unit-row">
										<span>Claim {claim.kind === 'threefold_repetition' ? 'threefold' : '50-move'} by declaring <strong>{claim.san}</strong></span>
										<button type="button" on:click={() => chessClaimDrawAfterSan(claim.kind, claim.san)}>Claim</button>
									</div>
								{/each}
							</div>
						{/if}
					{/if}
				</section>

				<section>
					<h2>Position</h2>
					<div class="fen-current">
						<input readonly value={currentFen} aria-label="Current FEN" />
						<button type="button" on:click={() => copyText(currentFen)}>Copy</button>
					</div>
					<form on:submit|preventDefault={loadFen} class="fen-loader">
						<input bind:value={fenDraft} spellcheck="false" aria-label="Load FEN" />
						<button type="submit">Load FEN</button>
					</form>
				</section>

				<section class="pgn-panel">
					<div class="panel-heading-row">
						<h2>PGN</h2>
						<button type="button" on:click={() => copyText(currentPgn)}>Copy</button>
					</div>
					<textarea readonly value={currentPgn} rows="9" aria-label="Current PGN"></textarea>
					<form on:submit|preventDefault={loadPgn} class="pgn-loader">
						<textarea bind:value={pgnDraft} rows="6" spellcheck="false" aria-label="Load PGN"></textarea>
						<button type="submit">Load PGN</button>
					</form>
				</section>
			{:else if game.ruleset !== 'go'}
				<section>
					<h2>Runtime proof</h2>
					<p class="panel-muted">
						This mode uses the same Monumla GameHandle, interaction choices, transactional history, undo/redo and board renderer as chess.
					</p>
				</section>
			{/if}
		</aside>
	</div>
{:else if error}
	<p class="runtime-error">{error}</p>
{:else}
	<p class="runtime-loading">{loading ? 'Loading Monumla WASM runtime…' : 'Runtime unavailable.'}</p>
{/if}

{#if error && game}
	<p class="runtime-error">{error}</p>
{/if}
