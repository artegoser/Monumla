<script lang="ts">
	import type { GameView, InteractionView } from '$lib/wasm/runtime';
	import type { AnalysisView } from '$lib/search/types';
	export let game: GameView;
	export let interaction: InteractionView;
	export let disabled = false;
	export let analysis: AnalysisView | null = null;
	export let onchoose: (id: string) => void;
	$: cells = Array.from({ length: game.width * game.height }, (_, i) => {
		const x = i % game.width,
			y = Math.floor(i / game.width);
		return {
			x,
			y,
			entity: game.entities.find((e) => e.position.x === x && e.position.y === y),
			input: interaction.choices.find((c) => c.position?.x === x && c.position?.y === y),
			candidate: analysis?.lines.findIndex((l) => l.path[0]?.x === x && l.path[0]?.y === y) ?? -1
		};
	});
</script>

<div
	class="alignment-board"
	style={`--columns:${game.width};--rows:${game.height}`}
	aria-label="Tic-tac-toe board"
>
	{#each cells as cell}
		{@const entity = cell.entity}
		{@const input = cell.input}
		{@const candidate = cell.candidate}
		<button
			type="button"
			class="alignment-cell"
			class:cross={entity?.owner === 1}
			class:circle={entity?.owner === 2}
			class:candidate={candidate >= 0}
			style:--hint-opacity={candidate === 0 ? 0.7 : candidate === 1 ? 0.4 : 0.2}
			aria-label={`${String.fromCharCode(65 + cell.x)}${cell.y + 1}${entity ? (entity.owner === 1 ? ' · Cross' : ' · Circle') : ' · Empty'}`}
			disabled={disabled || !input || !!entity}
			on:click={() => input && onchoose(input.id)}
		>
			{#if entity}<svg viewBox="0 0 100 100" aria-hidden="true"
					>{#if entity.owner === 1}<path d="M28 28 72 72M72 28 28 72" />{:else}<circle
							cx="50"
							cy="50"
							r="27"
						/>{/if}</svg
				>{:else if candidate >= 0}<span class="candidate-ring"></span>{/if}
		</button>
	{/each}
	<svg
		class="alignment-lines"
		viewBox={`0 0 ${game.width} ${game.height}`}
		preserveAspectRatio="none"
		aria-hidden="true"
	>
		{#each { length: game.width - 1 } as _, i}<line
				class="vertical-divider"
				x1={i + 1}
				y1="0"
				x2={i + 1}
				y2={game.height}
			/>{/each}
		{#each { length: game.height - 1 } as _, i}<line
				class="horizontal-divider"
				x1="0"
				y1={i + 1}
				x2={game.width}
				y2={i + 1}
			/>{/each}
	</svg>
</div>

<style>
	.alignment-board {
		position: relative;
		display: grid;
		grid-template-rows: repeat(var(--rows), minmax(0, 1fr));
		grid-template-columns: repeat(var(--columns), minmax(0, 1fr));
		aspect-ratio: var(--columns) / var(--rows);
		gap: 0;
		background: #5d645e;
		border: 2px solid #5d645e;
		border-radius: 8px;
		overflow: hidden;
		width: min(100%, calc(76vh * var(--columns) / var(--rows)));
	}
	.alignment-cell {
		position: relative;
		background: #2c3231;
		border: 0;
		display: grid;
		place-items: center;
		padding: 0;
		min-width: 0;
		min-height: 0;
		cursor: pointer;
		color: #91bdcd;
		width: 100%;
		height: 100%;
		overflow: hidden;
	}
	.alignment-cell:hover:not(:disabled) {
		background: #394441;
	}
	.alignment-cell:disabled {
		cursor: default;
		opacity: 1;
	}
	.alignment-cell.circle {
		color: #d9b585;
	}
	.alignment-cell svg {
		width: 100%;
		height: 100%;
		fill: none;
		stroke: currentColor;
		stroke-width: 7;
		stroke-linecap: round;
	}
	.alignment-lines {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		pointer-events: none;
	}
	.alignment-lines line {
		stroke: #7a8980;
		stroke-width: 2px;
		vector-effect: non-scaling-stroke;
	}
	.candidate-ring {
		width: 32%;
		height: 32%;
		border: 3px solid #a8c784;
		border-radius: 50%;
		opacity: var(--hint-opacity);
	}
</style>
