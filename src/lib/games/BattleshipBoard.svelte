<script lang="ts">
	import { stateScalar, type GameView, type InteractionView } from '$lib/wasm/runtime';
	import type { AnalysisView } from '$lib/search/types';
	import Icon from '$lib/ui/Icon.svelte';
	export let game: GameView;
	export let interaction: InteractionView;
	export let viewer = 1;
	export let disabled = false;
	export let analysis: AnalysisView | null = null;
	export let onchoose: (id: string) => void;
	let vertical = false;
	let hovered: number | null = null;
	const fleet = [4, 3, 3, 2, 2, 2, 1, 1, 1, 1];
	$: setup = stateScalar(game.status.details, 'phase') === 'setup';
	$: next = interaction.choices.find((c) => c.position && stateScalar(c.data, 'ship') != null);
	$: nextShip = next ? Number(stateScalar(next.data, 'ship')) : -1;
	$: length = fleet[nextShip] ?? 0;
	$: boards = [viewer, viewer === 1 ? 2 : 1].map((owner, index) => ({
		owner,
		index,
		cells: Array.from({ length: 100 }, (_, cell) => {
			const x = (owner === 1 ? 0 : 10) + (cell % 10),
				y = Math.floor(cell / 10);
			const result = stateScalar(game.status.details, `shot.${owner}.${cell}`);
			const hull = game.entities.find(
				(e) => e.owner === owner && e.position.x === x && e.position.y === y
			);
			const candidate =
				analysis?.lines.findIndex((l) => l.path[0]?.x === x && l.path[0]?.y === y) ?? -1;
			const input = interaction.choices.find(
				(c) =>
					c.position?.x === x &&
					c.position?.y === y &&
					(!setup || stateScalar(c.data, 'vertical') === vertical)
			);
			const choice =
				setup && hull
					? interaction.choices.find(
							(c) => c.option_key === `remove:${stateScalar(hull.state, 'ship')}`
						)
					: input;
			return { cell, result, hull, candidate, input, choice, enabled: !disabled && !!choice };
		})
	}));
	function rotateAt(cell: number | null) {
		if (!setup || disabled) return;
		const hull = cell == null ? null : boards[0]?.cells[cell]?.hull;
		if (hull) {
			const input = interaction.choices.find(
				(c) => c.option_key === `rotate:${stateScalar(hull.state, 'ship')}`
			);
			if (input) onchoose(input.id);
		} else if (length > 1) vertical = !vertical;
	}
	function keyboard(event: KeyboardEvent) {
		if (event.code !== 'KeyR' || event.ctrlKey || event.metaKey || event.altKey || event.repeat)
			return;
		if (
			event.target instanceof HTMLElement &&
			event.target.closest('input,textarea,select,[contenteditable="true"]')
		)
			return;
		if (setup && !disabled) {
			event.preventDefault();
			rotateAt(hovered);
		}
	}
	$: ghost =
		hovered == null || !boards[0]?.cells[hovered]?.input
			? []
			: Array.from({ length }, (_, i) => hovered! + i * (vertical ? 10 : 1));
</script>

<svelte:window on:keydown={keyboard} />

{#if setup}
	<div class="fleet-tools">
		<div>
			<strong>{length ? `Place your ${length}-deck ship` : 'Fleet complete'}</strong><span
				>{length
					? 'Click to place · click a ship to remove · R or right-click to rotate'
					: 'Confirm Ready when your fleet is set.'}</span
			>
		</div>
		<div class="ship-direction" role="group" aria-label="Ship direction">
			<button
				type="button"
				class:chosen={!vertical}
				aria-pressed={!vertical}
				disabled={disabled || !length}
				on:click={() => (vertical = false)}>Horizontal</button
			>
			<button
				type="button"
				class:chosen={vertical}
				aria-pressed={vertical}
				disabled={disabled || !length}
				on:click={() => (vertical = true)}>Vertical</button
			>
			<button
				type="button"
				disabled={disabled || (!length && !hovered)}
				aria-label="Rotate ship"
				title="Rotate ship (R). Hover a placed ship to rotate it."
				on:click={() => rotateAt(hovered)}><Icon name="rotate" /><kbd>R</kbd></button
			>
		</div>
	</div>
	<div class="fleet-inventory" aria-label="Fleet inventory">
		{#each fleet as size, index}<span
				class:placed={game.entities.some(
					(e) => e.owner === viewer && stateScalar(e.state, 'ship') === index
				)}
				class:next={index === nextShip}
				title={`${size}-deck ship`}
				aria-label={`${size}-deck ship${index === nextShip ? ' · next' : ''}`}
				>{#each { length: size } as _}<i></i>{/each}</span
			>{/each}
	</div>
{/if}
<div class="oceans">
	{#each boards as board}
		{@const owner = board.owner}
		{@const index = board.index}
		<section class="ocean" aria-label={index === 0 ? 'Your fleet' : 'Enemy waters'}>
			<div class="ocean-heading">
				<h2>{index === 0 ? 'Your fleet' : 'Enemy waters'}</h2>
				<span
					>{index === 0
						? `Player ${viewer}`
						: game.status.outcome
							? 'Fleet revealed'
							: 'Fog of war'}</span
				>
			</div>
			<div class="ocean-coordinates">
				<span></span>{#each 'ABCDEFGHIJ' as file}<span>{file}</span>{/each}
			</div>
			<div class="ocean-body">
				<div class="ocean-ranks">
					{#each { length: 10 } as _, row}<span>{row + 1}</span>{/each}
				</div>
				<div class="ocean-grid">
					{#each board.cells as tile}
						{@const cell = tile.cell}
						{@const result = tile.result}
						{@const hull = tile.hull}
						{@const candidate = tile.candidate}
						<button
							type="button"
							class="sea-cell"
							class:hull={!!hull}
							class:hit={result === 'hit'}
							class:sunk={result === 'sunk'}
							class:ghost={owner === viewer && ghost.includes(cell)}
							class:candidate={candidate >= 0}
							style:--hint-opacity={candidate === 0 ? 0.7 : candidate === 1 ? 0.4 : 0.2}
							data-cell={cell}
							data-ocean={owner}
							disabled={!tile.enabled}
							aria-label={`${index === 0 ? 'Your fleet' : 'Enemy waters'} ${String.fromCharCode(65 + (cell % 10))}${Math.floor(cell / 10) + 1}${result ? ` · ${result}` : ''}`}
							on:click={() => tile.choice && onchoose(tile.choice.id)}
							on:contextmenu={(event) => {
								if (setup) {
									event.preventDefault();
									if (index === 0) rotateAt(cell);
								}
							}}
							on:mouseenter={() => (hovered = index === 0 ? cell : null)}
							on:mouseleave={() => (hovered = null)}
							on:focus={() => (hovered = index === 0 ? cell : null)}
							on:blur={() => (hovered = null)}
						>
							{#if result === 'hit' || result === 'sunk'}<svg viewBox="0 0 32 32" aria-hidden="true"
									><path d="m10 10 12 12m0-12L10 22" /></svg
								>{:else if result === 'miss' || result === 'water'}<span
									class="water-mark"
									class:inferred={result === 'water'}
								></span>{:else if candidate >= 0}<span class="shot-candidate"></span>{/if}
						</button>
					{/each}
				</div>
			</div>
		</section>
	{/each}
</div>
<div class="sea-legend">
	<span><i class="legend-miss"></i>Miss</span><span><i class="legend-hit"></i>Hit</span><span
		><i class="legend-sunk"></i>Sunk</span
	><span>Ships cannot touch, even diagonally.</span>
</div>

<style>
	.ship-direction {
		display: flex;
		align-items: center;
		gap: 4px;
		flex-wrap: wrap;
		justify-content: flex-end;
	}
	.ship-direction :global(button.chosen) {
		background: #506d32;
		color: #fff;
	}
	.ship-direction kbd {
		font-size: 0.75rem;
		color: var(--muted);
	}

	.fleet-tools {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		margin: 0 0 16px;
	}
	.fleet-tools strong,
	.fleet-tools span {
		display: block;
	}
	.fleet-tools span {
		color: var(--muted);
		font-size: 0.85rem;
		margin-top: 5px;
	}
	.fleet-tools button {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-shrink: 0;
	}
	.fleet-inventory {
		display: flex;
		flex-wrap: wrap;
		gap: 12px 16px;
		min-height: 28px;
		margin-bottom: 24px;
	}
	.fleet-inventory > span {
		display: flex;
		gap: 2px;
		align-self: center;
		opacity: 0.35;
	}
	.fleet-inventory > span.placed {
		opacity: 1;
		color: #8ab1bd;
	}
	.fleet-inventory > span.next {
		opacity: 1;
		color: var(--accent);
	}
	.fleet-inventory i {
		display: block;
		width: 10px;
		height: 14px;
		background: currentColor;
		border-radius: 2px;
	}
	.oceans {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 24px;
	}
	.ocean-heading {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 8px;
		margin-bottom: 16px;
	}
	.ocean-heading h2 {
		font-size: 1rem;
		margin: 0;
		font-weight: 600;
	}
	.ocean-heading > span {
		font-size: 0.8rem;
		color: var(--muted);
	}
	.ocean-coordinates {
		display: grid;
		grid-template-columns: 18px repeat(10, minmax(0, 1fr));
		margin-bottom: 7px;
		color: #a4b8c0;
		text-align: center;
		font-size: 0.72rem;
	}
	.ocean-body {
		display: flex;
		gap: 0;
	}
	.ocean-ranks {
		width: 18px;
		flex-shrink: 0;
		display: grid;
		grid-template-rows: repeat(10, minmax(0, 1fr));
		color: #a4b8c0;
		font-size: 0.72rem;
	}
	.ocean-ranks span {
		display: grid;
		align-items: center;
	}
	.ocean-grid {
		display: grid;
		grid-template-columns: repeat(10, minmax(0, 1fr));
		gap: 1px;
		background: #37515b;
		border: 1px solid #48616a;
		border-radius: 4px;
		overflow: hidden;
		width: 100%;
		aspect-ratio: 1;
	}
	.sea-cell {
		position: relative;
		border: 0;
		border-radius: 0;
		padding: 0;
		background: #19313b;
		min-width: 0;
		min-height: 0;
		display: grid;
		place-items: center;
		cursor: crosshair;
		aspect-ratio: 1;
		color: #e6c0ad;
	}
	.sea-cell:disabled {
		opacity: 1;
		cursor: default;
	}
	.sea-cell:hover:not(:disabled) {
		background: #2b4b57;
	}
	.sea-cell.hull {
		background: #86abb8;
		box-shadow: inset 0 0 0 2px #19313b;
	}
	.sea-cell.ghost {
		background: #687e58;
	}
	.sea-cell.hit {
		background: #744439;
	}
	.sea-cell.sunk {
		background: #514b47;
	}
	.sea-cell svg {
		width: 100%;
		height: 100%;
		stroke: currentColor;
		stroke-width: 3;
		stroke-linecap: round;
		fill: none;
	}
	.water-mark {
		width: 16%;
		height: 16%;
		border-radius: 50%;
		background: #b3c6cc;
	}
	.water-mark.inferred {
		opacity: 0.35;
		width: 10%;
		height: 10%;
	}
	.shot-candidate {
		width: 58%;
		height: 58%;
		border: 2px solid #a8c784;
		opacity: var(--hint-opacity);
		border-radius: 50%;
	}
	.sea-legend {
		display: flex;
		gap: 20px;
		flex-wrap: wrap;
		font-size: 0.8rem;
		color: var(--muted);
		margin-top: 22px;
	}
	.sea-legend > span {
		display: flex;
		align-items: center;
		gap: 7px;
	}
	.sea-legend i {
		display: inline-block;
		width: 10px;
		height: 10px;
	}
	.legend-miss {
		background: #b3c6cc;
		border-radius: 50%;
		width: 4px !important;
		height: 4px !important;
	}
	.legend-hit {
		background: #bd7d63;
	}
	.legend-sunk {
		background: #77716b;
	}
	@media (max-width: 850px) {
		.oceans {
			gap: 18px;
		}
	}
	@media (max-width: 600px) {
		.oceans {
			grid-template-columns: 1fr;
			gap: 28px;
		}
		.fleet-tools {
			align-items: flex-start;
		}
		.fleet-tools span {
			max-width: 24ch;
		}
		.sea-legend {
			gap: 12px;
		}
	}
</style>
