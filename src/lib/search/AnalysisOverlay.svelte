<script lang="ts">
	import type { AnalysisLine } from './types';
	import BoardArrow from '$lib/board/BoardArrow.svelte';
	import { boardPoint } from '$lib/board/arrow';

	export let lines: AnalysisLine[] = [];
	export let selected = 0;
	export let width: number;
	export let height: number;
	export let orientation: 'white' | 'black';
	const opacity = [0.45, 0.33, 0.23];
	const thickness = [1.9, 1.35, 0.85];
	const colors = ['#003088', '#4a4a4a', '#4a4a4a'];
</script>

{#if lines.some((line) => line.path.length)}
	<svg class="search-overlay" viewBox="0 0 100 100" aria-label="Suggested moves" role="img">
		{#each lines
			.slice(0, 3)
			.map((line, rank) => ({ line, rank }))
			.reverse() as { line, rank }}
			{#if line.path.length}
				<g class="candidate-arrow" data-rank={rank + 1} opacity={opacity[rank]}>
					{#if line.path.length === 1}
						<circle
							cx={boardPoint(line.path[0], orientation, width, height).x}
							cy={boardPoint(line.path[0], orientation, width, height).y}
							r={32 / width}
							fill="none"
							stroke={rank === selected ? colors[0] : colors[rank]}
							stroke-width={thickness[rank]}
						/>
					{:else}
						{#each line.path.slice(1) as to, i}
							<BoardArrow
								from={line.path[i]}
								{to}
								{orientation}
								{width}
								{height}
								color={rank === selected ? colors[0] : colors[rank]}
								thickness={thickness[rank]}
							/>
						{/each}
					{/if}
				</g>
			{/if}
		{/each}
	</svg>
{/if}
