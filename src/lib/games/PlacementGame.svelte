<script lang="ts">
	import { onMount } from 'svelte';
	import {
		LocalGame,
		stateScalar,
		type GameView,
		type InteractionView,
		type HistoryTurnView
	} from '$lib/wasm/runtime';
	import type { AnalysisView, BestAction, MoveAssessment } from '$lib/search/types';
	import AnalysisPanel from '$lib/search/AnalysisPanel.svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import GameReview from '$lib/search/GameReview.svelte';
	import MoveBadge from '$lib/search/MoveBadge.svelte';
	import { describeAssessment } from '$lib/search/assessment';
	import AlignmentBoard from './AlignmentBoard.svelte';
	import BattleshipBoard from './BattleshipBoard.svelte';
	export let ruleset: 'battleship' | 'tictactoe';
	export let mode: 'play' | 'analysis';
	let runtime: LocalGame | null = null;
	let game: GameView | null = null;
	let interaction: InteractionView;
	let history: HistoryTurnView[] = [];
	let error = '';
	let loading = true;
	let revision = 0;
	let pauseRevision = 0;
	let analysis: AnalysisView | null = null;
	let showHints = true;
	let viewer = 1;
	let botSide: number | null = 2;
	let bothBots = false;
	let local = false;
	let control = '';
	let handover = false;
	let preset = '3,3,3';
	let customOpen = false;
	let reviewResults: Record<string, MoveAssessment> = {};
	let analyzedPositions: Record<string, AnalysisView> = {};
	$: reviewKeys = history.map((_, i) => historyKey(i + 1));
	$: beforeKeys = history.map((_, i) => historyKey(i));
	$: currentAssessment = reviewResults[historyKey(history.length)];
	$: currentGrade = describeAssessment(currentAssessment);
	function historyKey(length: number) {
		return (
			ruleset +
			':' +
			history
				.slice(0, length)
				.map((t) => `${t.actor}:${t.notation}`)
				.join('/')
		);
	}
	function receiveAnalysis(value: AnalysisView | null) {
		analysis = value;
		if (value?.best_action?.position_key === currentKey)
			analyzedPositions = { ...analyzedPositions, [historyKey(history.length)]: value! };
	}

	let columns = 3;
	let rows = 3;
	let connect = 3;
	let mounted = false;
	$: currentKey = game && runtime && revision >= 0 ? runtime.analysisPositionKey() : '';
	$: hints =
		mode === 'analysis' &&
		showHints &&
		runtime &&
		analysis?.best_action?.position_key === currentKey
			? analysis
			: null;
	$: blocked =
		handover ||
		!!game?.status.outcome ||
		(mode === 'play' && (bothBots || botSide === game?.active_players[0]));
	$: setup = game && stateScalar(game.status.details, 'phase') === 'setup';
	$: actions =
		interaction?.choices.filter((c) =>
			['arrange', 'clear', 'ready'].includes(c.option_key ?? '')
		) ?? [];
	$: if (
		mode === 'analysis' &&
		mounted &&
		game &&
		ruleset === 'battleship' &&
		viewer !== game.active_players[0]
	) {
		viewer = game.active_players[0];
		handover = false;
		refresh();
	}
	function refresh() {
		if (!runtime) return;
		runtime.setPerspective(viewer);
		game = runtime.view();
		interaction = runtime.interaction();
		history = runtime.history();
		revision += 1;
	}
	async function start() {
		loading = true;
		error = '';
		try {
			const next =
				ruleset === 'battleship'
					? await LocalGame.createBattleship()
					: await LocalGame.createTicTacToe(columns, rows, connect);
			if (!mounted) {
				next.dispose();
				return;
			}
			const old = runtime;
			runtime = next;
			handover = false;
			analysis = null;
			reviewResults = {};
			analyzedPositions = {};
			refresh();
			old?.dispose();
		} catch (cause) {
			error = String(cause);
		} finally {
			loading = false;
		}
	}
	function perform(action: () => unknown) {
		if (!runtime || !game) return;
		const before = game.active_players[0];
		try {
			action();
			error = '';
			const active = runtime.view().active_players[0];
			if (
				ruleset === 'battleship' &&
				active !== before &&
				local &&
				mode === 'play' &&
				!runtime.view().status.outcome
			)
				handover = true;
			if (ruleset === 'battleship' && mode === 'analysis') viewer = active;
			refresh();
		} catch (cause) {
			error = String(cause);
		}
	}
	function choose(id: string) {
		if (!blocked) perform(() => runtime!.choose(id));
	}
	function play(best: BestAction) {
		if (!handover) perform(() => runtime!.applyBestAction(best));
	}
	function controls(
		side: number | null,
		human: number,
		_white: number,
		both: boolean,
		_paused: boolean
	) {
		const signature = `${mode}:${side}:${human}:${both}`;
		botSide = side;
		bothBots = both;
		local = side == null && !both && mode === 'play';
		if (control === signature) return;
		control = signature;
		if (ruleset === 'battleship' && runtime && game) {
			if (local) {
				handover = history.length > 0;
				if (!handover) viewer = game.active_players[0];
			} else {
				viewer = mode === 'analysis' ? game.active_players[0] : human;
				handover = false;
			}
			refresh();
		}
	}
	function reveal() {
		if (game) {
			viewer = game.active_players[0];
			handover = false;
			refresh();
		}
	}
	function historyStep(forward: boolean) {
		pauseRevision += 1;
		perform(() => (forward ? runtime!.redo() : runtime!.undo()));
	}
	function changePreset() {
		if (preset === 'custom') {
			customOpen = true;
			return;
		}
		customOpen = false;
		[columns, rows, connect] = preset.split(',').map(Number);
		void start();
	}
	onMount(() => {
		mounted = true;
		void start();
		return () => {
			mounted = false;
			runtime?.dispose();
		};
	});
</script>

<main class="placement-layout" class:sea-layout={ruleset === 'battleship'}>
	<div class="placement-content">
		{#if loading}<p class="loading-state" role="status">Preparing the board…</p>{/if}
		{#if game && runtime}
			<div class="placement-heading">
				<div>
					<h1>
						{ruleset === 'battleship'
							? 'Battleship'
							: `Connect ${stateScalar(game.status.details, 'connect')}`}
					</h1>
					<p>
						{ruleset === 'battleship'
							? 'Your fleet stays yours. Find theirs.'
							: `${game.width} × ${game.height} · Cross goes first`}
					</p>
				</div>
				<button
					type="button"
					class="icon-button"
					aria-label="New game"
					title="New game"
					on:click={start}><Icon name="reset" /></button
				>
			</div>
			{#if ruleset === 'tictactoe'}
				<div class="board-presets">
					<label
						>Board<select
							aria-label="Tic-tac-toe board"
							bind:value={preset}
							on:change={changePreset}
						>
							<option value="3,3,3">3 × 3 · Classic</option><option value="4,4,3"
								>4 × 4 · Connect 3</option
							>
							<option value="5,5,4">5 × 5 · Connect 4</option><option value="7,5,5"
								>7 × 5 · Connect 5</option
							>
							<option value="custom">Custom board</option>
						</select></label
					>
					<button
						class="custom-toggle"
						type="button"
						aria-expanded={customOpen}
						aria-controls="custom-board-fields"
						on:click={() => (customOpen = !customOpen)}><Icon name="settings" />Custom board</button
					>
				</div>
				{#if customOpen}
					<form
						id="custom-board-fields"
						class="custom-board"
						on:submit|preventDefault={() => {
							preset = 'custom';
							void start();
						}}
					>
						<label
							>Columns<input
								aria-label="Columns"
								type="number"
								min="3"
								max="9"
								bind:value={columns}
								required
							/></label
						>
						<label
							>Rows<input
								aria-label="Rows"
								type="number"
								min="3"
								max="9"
								bind:value={rows}
								required
							/></label
						>
						<label
							>Connect<input
								aria-label="Connect"
								type="number"
								min="3"
								max={Math.max(columns, rows)}
								bind:value={connect}
								required
							/></label
						>
						<button type="submit">Create board</button>
					</form>
				{/if}
			{/if}
			{#if handover}
				<section class="handover" aria-label="Pass the device">
					<Icon name="user" />
					<h2>Pass to Player {game.active_players[0]}</h2>
					<p>Keep your fleet hidden. Open the board when the other player has looked away.</p>
					<button type="button" class="primary-action" on:click={reveal}
						>I’m Player {game.active_players[0]} · show my board</button
					>
				</section>
			{:else if ruleset === 'battleship'}
				<BattleshipBoard
					{game}
					{interaction}
					{viewer}
					disabled={blocked}
					analysis={hints}
					onchoose={choose}
				/>
				{#if setup}<div class="setup-actions">
						{#each actions as action}<button
								type="button"
								class:primary-action={action.option_key === 'ready'}
								disabled={blocked}
								on:click={() => choose(action.id)}>{action.label}</button
							>{/each}
					</div>{/if}
			{:else}<AlignmentBoard
					{game}
					{interaction}
					disabled={blocked}
					analysis={hints}
					onchoose={choose}
				/>{/if}
			<p class="placement-status" aria-live="polite">{game.status.text}</p>
		{/if}
		{#if error}<p class="game-error" role="alert">{error}</p>{/if}
	</div>
	{#if game && runtime}
		<aside class="game-panel placement-panel">
			<AnalysisPanel
				{runtime}
				{game}
				{mode}
				{revision}
				{pauseRevision}
				bind:showArrows={showHints}
				onplay={play}
				onanalysis={receiveAnalysis}
				oncontrolchange={controls}
			/>
			<section class="placement-history">
				<div class="panel-heading-row">
					<h2>{ruleset === 'battleship' ? 'Logbook' : 'Moves'}</h2>
					<span>{history.length} turns</span>
				</div>
				{#if mode === 'analysis'}
					<div class="placement-review">
						{#if currentAssessment}<div
								class="placement-move-grade"
								data-quality={currentGrade.tone}
								style:--quality-color={currentGrade.color}
							>
								<MoveBadge assessment={currentAssessment} />
								<div>
									<strong>{history[history.length - 1]?.notation} · {currentGrade.name}</strong
									><small
										>{currentAssessment.samples > 0
											? `Sampled estimate · ${currentAssessment.samples} layouts`
											: `Engine estimate · depth ${currentAssessment.depth}`}</small
									>
								</div>
							</div>{/if}
						<GameReview
							{runtime}
							{revision}
							{history}
							keys={reviewKeys}
							{beforeKeys}
							analyses={analyzedPositions}
							results={reviewResults}
							onassessment={(key, value) => (reviewResults = { ...reviewResults, [key]: value })}
						/>
					</div>
				{/if}
				<div class="logbook" role="log" aria-label="Game history">
					{#if history.length === 0}<p>
							{ruleset === 'battleship'
								? 'Place all ten ships, then confirm Ready.'
								: 'Place your first mark.'}
						</p>{/if}{#each history as turn}<div class="log-row">
							<span>{turn.index + 1}.</span><span class:player-one={turn.actor === 1}
								>P{turn.actor}</span
							><strong>{turn.notation}</strong
							>{#if mode === 'analysis' && reviewResults[reviewKeys[turn.index]]}<MoveBadge
									assessment={reviewResults[reviewKeys[turn.index]]}
									title={reviewResults[reviewKeys[turn.index]].label}
								/>{/if}
						</div>{/each}
				</div>
				<div class="history-controls">
					<button
						type="button"
						class="icon-button"
						aria-label="Previous move"
						disabled={!game.can_undo || handover}
						on:click={() => historyStep(false)}><Icon name="back" /></button
					><button
						type="button"
						class="icon-button"
						aria-label="Next move"
						disabled={!game.can_redo || handover}
						on:click={() => historyStep(true)}><Icon name="next" /></button
					>
				</div>
			</section>
			<p class="placement-note">
				{ruleset === 'battleship'
					? '10 × 10 · 4, 3, 3, 2, 2, 2, 1, 1, 1, 1 decks. A hit keeps your turn. Sunk ships reveal the surrounding water.'
					: `Make an unbroken line of ${stateScalar(game.status.details, 'connect')} marks, horizontally, vertically or diagonally.`}
			</p>
		</aside>
	{/if}
</main>

<style>
	.placement-move-grade {
		display: flex;
		align-items: center;
		gap: 10px;
		border: 1px solid var(--quality-color);
		padding: 12px;
		margin-bottom: 12px;
		min-height: 68px;
	}
	.placement-move-grade strong {
		display: block;
		font-size: 0.85rem;
		color: var(--quality-color);
		font-weight: 550;
	}
	.placement-move-grade small {
		display: block;
		margin-top: 5px;
		font-size: 0.75rem;
		color: var(--muted);
	}

	.custom-toggle {
		display: flex;
		align-items: center;
		gap: 8px;
		min-height: 38px;
	}
	.placement-review {
		padding: 10px 18px 0;
		min-height: 60px;
	}
	.custom-board label {
		display: flex;
		flex-direction: column;
		gap: 6px;
		font-size: 0.8rem;
		color: var(--muted);
	}

	.placement-layout {
		max-width: 1190px;
		margin: 8px auto 40px;
		padding: 0 1.5rem;
		display: grid;
		grid-template-columns: minmax(0, 1fr) 320px;
		gap: 24px;
		align-items: start;
	}
	.placement-layout.sea-layout {
		max-width: 1190px;
	}
	.placement-content {
		min-width: 0;
	}
	.placement-heading {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 20px;
		margin: 6px 0 24px;
	}
	.placement-heading h1 {
		font-size: 1.6rem;
		font-weight: 550;
		margin: 0;
	}
	.placement-heading p {
		font-size: 0.9rem;
		color: var(--muted);
		margin: 7px 0 0;
	}
	.placement-content :global(button) {
		background: #3c3a35;
		color: inherit;
		border: 0;
		border-radius: 4px;
		cursor: pointer;
		padding: 0.6rem 0.85rem;
	}
	.placement-content :global(button:disabled) {
		cursor: default;
		opacity: 0.4;
	}
	.placement-content :global(.sea-cell:disabled),
	.placement-content :global(.alignment-cell:disabled),
	.placement-content :global(.sea-cell),
	.placement-content :global(.alignment-cell) {
		padding: 0;
		border-radius: 0;
		opacity: 1;
	}
	.placement-content :global(.sea-cell) {
		background: #19313b;
	}
	.placement-content :global(.sea-cell.hull) {
		background: #86abb8;
	}
	.placement-content :global(.sea-cell.hit) {
		background: #744439;
	}
	.placement-content :global(.sea-cell.sunk) {
		background: #514b47;
	}
	.placement-content :global(.sea-cell.ghost) {
		background: #687e58;
	}
	.placement-content :global(.alignment-cell) {
		background: #2c3231;
	}
	.placement-content :global(button:hover:not(:disabled)) {
		filter: brightness(1.13);
	}
	.placement-content :global(.primary-action) {
		background: #506d32;
		color: #fff;
	}
	.placement-status {
		margin: 20px 0;
		color: var(--muted);
		font-size: 0.9rem;
		min-height: 24px;
	}
	.setup-actions {
		display: flex;
		gap: 10px;
		flex-wrap: wrap;
		margin-top: 24px;
	}
	.board-presets {
		display: flex;
		justify-content: space-between;
		gap: 16px;
		align-items: end;
		margin-bottom: 20px;
	}
	.board-presets label {
		display: flex;
		flex-direction: column;
		gap: 6px;
		font-size: 0.8rem;
		color: var(--muted);
	}
	.board-presets select {
		color: #d6d3ce;
	}

	.custom-board {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr)) auto;
		gap: 12px;
		align-items: end;
		padding: 16px 0 20px;
		border-top: 1px solid var(--line);
	}
	.custom-board input {
		width: 100%;
	}
	.handover {
		min-height: 380px;
		background: #19313b;
		border-radius: 6px;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-direction: column;
		padding: 32px;
		text-align: center;
	}
	.handover p {
		max-width: 40ch;
		color: #b7c9cf;
		margin: 4px 0 24px;
		line-height: 1.6;
	}
	.handover :global(svg) {
		width: 36px;
		height: 36px;
		color: #a8c784;
	}
	.placement-panel {
		width: 100%;
		margin-top: 8px;
	}
	.placement-history {
		border-top: 1px solid var(--line);
		padding-top: 20px;
	}
	.placement-history .panel-heading-row {
		padding: 0 18px;
	}
	.placement-history h2 {
		font-size: 0.9rem;
		font-weight: 600;
		margin: 0;
	}
	.placement-history .panel-heading-row > span {
		font-size: 0.75rem;
		color: var(--muted);
	}
	.logbook {
		height: 300px;
		overflow: auto;
		scrollbar-width: thin;
		scrollbar-color: #5c5a52 #2c2b28;
		margin-top: 16px;
		padding: 0 16px;
	}
	.logbook > p {
		font-size: 0.85rem;
		line-height: 1.6;
		color: var(--muted);
	}
	.log-row {
		display: grid;
		grid-template-columns: 24px 26px minmax(0, 1fr) 24px;
		gap: 8px;
		align-items: baseline;
		padding: 9px 0;
		font-size: 0.85rem;
		border-bottom: 1px solid var(--line);
	}
	.log-row > span {
		color: var(--muted);
		font-variant-numeric: tabular-nums;
	}
	.log-row > span.player-one {
		color: #91bdcd;
	}
	.log-row strong {
		font-weight: 400;
		overflow-wrap: anywhere;
	}
	.placement-note {
		padding: 18px;
		color: var(--muted);
		font-size: 0.8rem;
		line-height: 1.6;
		border-top: 1px solid var(--line);
		margin: 0;
	}
	.history-controls {
		display: flex;
		justify-content: center;
		gap: 24px;
		padding: 8px;
		border-top: 1px solid var(--line);
	}
	.game-error {
		color: #eca28d;
	}
	@media (max-width: 1000px) {
		.sea-layout {
			grid-template-columns: minmax(0, 1fr) 290px;
			gap: 18px;
		}
		.sea-layout :global(.oceans) {
			grid-template-columns: 1fr;
		}
	}
	@media (max-width: 760px) {
		.placement-layout,
		.placement-layout.sea-layout {
			grid-template-columns: minmax(0, 1fr);
			padding: 0 16px;
			gap: 12px;
		}
		.placement-panel {
			margin-top: 0;
		}
		.placement-heading {
			margin-top: 8px;
		}
		.board-presets {
			align-items: start;
		}
		.logbook {
			height: 220px;
		}
		.sea-layout :global(.oceans) {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}
	@media (max-width: 540px) {
		.custom-board {
			grid-template-columns: repeat(3, minmax(0, 1fr));
		}
		.custom-board > button {
			grid-column: 1/-1;
		}
		.sea-layout :global(.oceans) {
			grid-template-columns: 1fr;
		}
		.board-presets {
			flex-wrap: wrap;
		}
	}
</style>
