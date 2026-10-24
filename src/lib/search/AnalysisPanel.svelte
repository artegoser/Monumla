<script lang="ts">
	import { onMount } from 'svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import PlacementOptions from '$lib/games/PlacementOptions.svelte';
	import { displayLines, displayScore } from './presentation';
	import { stateScalar, type ChoiceView } from '$lib/wasm/runtime';
	import { SearchCancelled, SearchClient } from './client';
	import type { AnalysisView, BestAction } from './types';
	import type { GameView, LocalGame } from '$lib/wasm/runtime';

	export let onpreview: (choice: ChoiceView | null) => void = () => {};
	export let runtime: LocalGame;
	export let game: GameView;
	export let mode: 'play' | 'analysis';
	export let revision: number;
	export let pauseRevision = 0;
	export let turnInProgress = false;
	export let selectedLine = 0;
	export let showArrows = true;
	export let showEvaluation = true;
	export let showAnnotations = true;
	export let showCoordinates = true;
	export let onplay: (best: BestAction) => void;
	export let onanalysis: (analysis: AnalysisView | null) => void;
	export let oncontrolchange: (
		side: number | null,
		human: number,
		white: number,
		both: boolean,
		paused: boolean
	) => void;

	const engine = new SearchClient();
	let mounted = false;
	let opponent = 'bot';
	let humanSide = 1;
	let timeMs = 1000;
	let lineCount = 3;
	let paused = false;
	let thinking = false;
	let analysis: AnalysisView | null = null;
	let analysisKey = '';
	let currentKey = '';
	const cache = new Map<string, AnalysisView>();
	$: displayedLines = displayLines(analysis, lineCount);
	$: liveScore = displayScore(analysis);
	$: live = analysis?.iteration;
	$: isCurrent = analysis != null && analysisKey === currentKey;
	let failure: string | null = null;
	let job = 0;
	let scheduled = '';
	let seenRuntime: LocalGame | null = null;
	let seenRevision = -1;
	let seenMode = mode;
	let seenPause = pauseRevision;
	let settingsOpen = false;

	$: whiteSide = game.ruleset === 'go' ? 2 : 1;
	$: setup = game.ruleset === 'battleship' && stateScalar(game.status.details, 'phase') === 'setup';
	$: placement = game.ruleset === 'battleship' || game.ruleset === 'tictactoe';
	$: firstLabel =
		game.ruleset === 'battleship' ? 'Player 1' : game.ruleset === 'tictactoe' ? 'Cross' : 'White';
	$: secondLabel =
		game.ruleset === 'battleship' ? 'Player 2' : game.ruleset === 'tictactoe' ? 'Circle' : 'Black';
	$: botSide =
		mode === 'play'
			? opponent === 'bots'
				? (game.active_players[0] ?? null)
				: opponent === 'bot'
					? humanSide === 1
						? 2
						: 1
					: null
			: null;
	$: oncontrolchange(botSide, humanSide, whiteSide, opponent === 'bots' && mode === 'play', paused);
	$: botTurn = botSide != null && game.active_players.includes(botSide) && !game.status.outcome;
	$: if (pauseRevision !== seenPause) {
		seenPause = pauseRevision;
		if (mode === 'play') paused = true;
	}
	$: reconcile(
		mounted,
		runtime,
		revision,
		mode,
		botSide,
		timeMs,
		lineCount,
		paused,
		turnInProgress,
		setup
	);

	function cancel() {
		job += 1;
		engine.cancel();
		thinking = false;
	}
	function reconcile(
		ready: boolean,
		current: LocalGame,
		position: number,
		currentMode: string,
		computer: number | null,
		duration: number,
		lines: number,
		suspended: boolean,
		partial: boolean,
		preparing: boolean
	) {
		if (!ready) return;
		const switched = current !== seenRuntime || currentMode !== seenMode;
		if (current !== seenRuntime) {
			engine.dispose();
			cache.clear();
			analysis = null;
			analysisKey = '';
			currentKey = '';
		}
		if (current !== seenRuntime || currentMode !== seenMode) {
			paused = false;
			suspended = false;
			seenRuntime = current;
			seenMode = mode;
		}
		if (position !== seenRevision || switched) {
			currentKey = current.analysisPositionKey();
			const retained = cache.get(currentKey);
			if (retained) {
				analysis = retained;
				analysisKey = currentKey;
			}
			onanalysis(retained ?? null);
			selectedLine = 0;
			seenRevision = position;
		}
		const key = `${position}:${currentMode}:${computer}:${duration}:${lines}:${suspended}:${partial}`;
		if (key === scheduled) return;
		cancel();
		scheduled = key;
		failure = null;
		if (partial || suspended) return;
		if (currentMode === 'analysis' && preparing) {
			onanalysis(null);
			return;
		}
		if (currentMode === 'analysis') void start(false);
		else if (computer != null && game.active_players.includes(computer) && !game.status.outcome)
			void start(true);
	}
	async function start(playAfter: boolean, duration = timeMs) {
		cancel();
		const request = job;
		const position = revision;
		const current = runtime;
		const sourceKey = current.analysisPositionKey();
		thinking = true;
		try {
			const accept = (value: AnalysisView) => {
				if (request !== job || position !== revision || current !== runtime) return;
				if (value.score == null && !value.iteration) return;
				const previous = cache.get(sourceKey);
				if (previous && previous.depth > value.depth) return;
				if (value.score != null) {
					cache.delete(sourceKey);
					cache.set(sourceKey, value);
					if (cache.size > 128) cache.delete(cache.keys().next().value!);
				}
				analysis = value;
				analysisKey = sourceKey;
				if (selectedLine >= value.lines.length) selectedLine = 0;
				if (!playAfter) onanalysis(value);
			};
			const result = await engine.analyze(
				current.searchSnapshot(),
				{
					maxDepth: 32,
					maxNodes: 50_000_000,
					timeMs: playAfter ? duration : 600_000,
					lines: playAfter ? 1 : lineCount,
					forcedMove: playAfter
				},
				accept
			);
			if (request !== job || position !== revision || current !== runtime) return;
			accept(result);
			thinking = false;
			if (playAfter) {
				if (result.best_action) onplay(result.best_action);
				else if (!game.status.outcome && duration < 30_000)
					void start(true, Math.min(30_000, duration * 3));
				else if (!game.status.outcome)
					throw new Error('No search iteration completed. Increase thinking time and retry.');
			}
		} catch (cause) {
			if (request !== job || cause instanceof SearchCancelled) return;
			thinking = false;
			failure = cause instanceof Error ? cause.message : String(cause);
			paused = true;
		}
	}
	function togglePause() {
		paused = !paused;
		scheduled = '';
	}
	function whiteScore(score: number) {
		return game.ruleset === 'battleship'
			? score
			: score * (analysis?.perspective === whiteSide ? 1 : -1);
	}
	function scoreText(score: number | null) {
		if (score == null) return '…';
		const value = whiteScore(score);
		if (Math.abs(value) > 90_000) {
			const turns = Math.ceil((100_000 - Math.abs(value)) / 2);
			return turns > 0 && game.ruleset === 'chess'
				? `${value > 0 ? '+' : '−'}M${turns}`
				: value > 0
					? 'White wins'
					: 'Black wins';
		}
		return `${value > 0 ? '+' : ''}${(value / 100).toFixed(2)}`;
	}
	onMount(() => {
		mounted = true;
		return () => {
			cancel();
			engine.dispose();
			oncontrolchange(null, humanSide, whiteSide, false, true);
		};
	});
</script>

<section
	class="engine-panel"
	data-searching-depth={live?.depth ?? analysis?.depth ?? 0}
	data-provisional={!!live}
	data-analysis-depth={isCurrent ? (analysis?.depth ?? 0) : 0}
	data-analysis-pending={!isCurrent}
	aria-label={mode === 'analysis' ? 'Position analysis' : 'Computer opponent'}
>
	<div class="panel-heading-row">
		<h2>{mode === 'analysis' ? (setup ? 'Fleet placement' : 'Position analysis') : 'Your game'}</h2>
		<div class="panel-tools">
			{#if (mode === 'analysis' && !setup) || botTurn || paused}
				<button
					class="icon-button"
					type="button"
					on:click={togglePause}
					aria-label={paused ? 'Resume search' : 'Pause search'}
					title={paused ? 'Resume search' : 'Pause search'}
					><Icon name={paused ? 'play' : 'pause'} /></button
				>
			{/if}
			<button
				class="icon-button"
				type="button"
				on:click={() => (settingsOpen = !settingsOpen)}
				aria-label="Engine settings"
				aria-expanded={settingsOpen}><Icon name="settings" /></button
			>
		</div>
	</div>
	{#if mode === 'play'}
		<div class="engine-settings">
			<label
				>Opponent<select bind:value={opponent} on:change={() => (paused = false)}
					><option value="bot">Play the computer</option><option value="bots"
						>Computer vs computer</option
					><option value="local">A friend · local</option></select
				></label
			>
			{#if opponent === 'bot'}
				<label
					>Your side<select bind:value={humanSide} on:change={() => (paused = false)}
						><option value={whiteSide}>{firstLabel}</option><option value={whiteSide === 1 ? 2 : 1}
							>{secondLabel}</option
						></select
					></label
				>
			{/if}
		</div>
		{#if settingsOpen && opponent !== 'local'}
			<label class="field-label"
				>Thinking time<select bind:value={timeMs}
					><option value={300}>Quick · 0.3 seconds</option><option value={1000}
						>Balanced · 1 second</option
					><option value={3000}>Careful · 3 seconds</option><option value={10000}
						>Extended · 10 seconds</option
					></select
				></label
			>
		{/if}
		<p class="engine-status" aria-live="polite">
			<span class="status-dot" class:busy={thinking}></span>{game.status.outcome
				? game.status.text
				: paused
					? 'Computer paused'
					: thinking
						? 'Computer is thinking…'
						: opponent === 'bots'
							? 'Computers are playing'
							: opponent === 'bot'
								? 'Your move'
								: 'Pass the board to your opponent'}
		</p>
	{:else if setup}
		<PlacementOptions choices={runtime.interaction().choices} {onpreview} />
	{:else}
		{#if settingsOpen}
			<div class="analysis-settings">
				<label class="field-label"
					>Best lines<select bind:value={lineCount}
						><option value={1}>1 line</option><option value={3}>3 lines</option><option value={5}
							>5 lines</option
						></select
					></label
				>
				<label
					><input type="checkbox" bind:checked={showArrows} />
					{placement ? 'Candidate squares' : 'Best-move arrows'}</label
				>
				{#if !placement}
					<label><input type="checkbox" bind:checked={showEvaluation} /> Evaluation bar</label>
					<label><input type="checkbox" bind:checked={showAnnotations} /> Move assessments</label>
					<label><input type="checkbox" bind:checked={showCoordinates} /> Board coordinates</label>
				{/if}
			</div>
		{/if}
		<div class="analysis-summary" class:previous-position={!isCurrent && analysis != null}>
			<strong class="score" aria-label="Position evaluation">{scoreText(liveScore)}</strong>
			<div>
				<span
					>{game.ruleset === 'battleship'
						? `Player ${analysis?.perspective ?? game.active_players[0]} perspective`
						: `${firstLabel} perspective`}</span
				><small>
					{turnInProgress
						? 'Complete the current move'
						: !isCurrent && analysis
							? 'Previous position · searching'
							: thinking
								? live
									? `Provisional · depth ${live.depth}${live.root_total ? ` · ${live.root_done}/${live.root_total} moves` : ''}${live.line_index > 1 ? ` · PV ${live.line_index}` : ''}`
									: 'Searching deeper'
								: paused
									? `Analysis paused${live ? ` · unfinished depth ${live.depth}` : ''}`
									: 'Search finished'}{isCurrent && analysis && !live
						? analysis.samples > 0
							? ` · ${analysis.samples} sampled layouts`
							: ` · depth ${analysis.depth}`
						: ''}
				</small>
			</div>
			<span class="status-dot" class:busy={thinking}></span>
		</div>
		<div
			class="best-lines"
			aria-label="Best continuations"
			style:grid-template-rows={`repeat(${lineCount}, 42px)`}
		>
			{#each Array.from({ length: lineCount }) as _, index}
				{@const line = displayedLines[index]}
				{#if line}
					<button
						type="button"
						class:chosen={selectedLine === index && isCurrent}
						style:--candidate-color={index === selectedLine ? '#82a6ce' : '#aaa79e'}
						on:click={() => (selectedLine = index)}
						on:mouseenter={() => (selectedLine = index)}
						on:mouseleave={() => (selectedLine = 0)}
						on:focus={() => (selectedLine = index)}
						on:blur={() => (selectedLine = 0)}
						disabled={!isCurrent || turnInProgress}
						aria-pressed={selectedLine === index && isCurrent}
						title={isCurrent
							? `${line.moves.join('  ')} · Completed depth ${line.depth}`
							: 'Previous position; waiting for current analysis'}
					>
						<span class="line-rank">{index + 1}</span><strong>{scoreText(line.score)}</strong><span
							class="line-moves">{line.moves.join('  ')}</span
						>
					</button>
				{:else}
					<div class="line-placeholder">
						<span>{index + 1}</span><span>…</span><span
							>{index === 0
								? game.status.outcome
									? game.status.text
									: turnInProgress
										? 'Complete the current move'
										: paused
											? 'Resume analysis'
											: 'Searching this position…'
								: 'Awaiting a completed line'}</span
						>
					</div>
				{/if}
			{/each}
		</div>
		<p
			class="working-line"
			aria-label="Unfinished search candidate"
			title="An unfinished root search can change as other moves are checked; it does not replace the completed ranking."
		>
			{#if live?.line}
				<span>Working · depth {live.depth}</span>
				<strong
					>{live.bound === 'exact'
						? ''
						: live.bound === 'estimate'
							? '≈ '
							: (live.bound === 'lower') ===
								  (game.ruleset === 'battleship' || analysis?.perspective === whiteSide)
								? '≥ '
								: '≤ '}{scoreText(live.line.score)}</strong
				>
				<span class="working-moves">{live.line.moves.join('  ')}</span>
			{:else}
				<span>{thinking ? 'Checking candidates…' : 'Completed ranking above'}</span>
			{/if}
		</p>
		<p class="search-details">
			{analysis && isCurrent
				? `${analysis.nodes.toLocaleString()} nodes · ${(analysis.nodes_per_second / 1000).toFixed(1)}k nodes/s · ${(analysis.elapsed_ms / 1000).toFixed(1)}s`
				: 'Waiting for a completed search iteration'}
		</p>
		<p class="evaluation-note">
			{game.ruleset === 'battleship'
				? 'Sampled one-shot estimate · legal fleet completions'
				: 'Position estimate · not a win probability'}{game.ruleset === 'go'
				? ' or territory count'
				: ''}.
		</p>
	{/if}
	{#if failure}<p class="engine-error" role="alert">{failure}</p>{/if}
</section>
