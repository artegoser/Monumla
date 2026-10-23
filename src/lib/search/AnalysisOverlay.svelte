<script lang="ts">
	import type { AnalysisLine } from './types';
	import type { PositionView } from '$lib/wasm/runtime';
	export let lines: AnalysisLine[] = [];
	export let selected = 0;
	export let width: number;
	export let height: number;
	export let orientation: 'white' | 'black';
	const colors = ['#7db869', '#6ba9cf', '#b294d0', '#d6ae60', '#d3847b'];
	function point(p: PositionView) {
		return {
			x: (((orientation === 'white' ? p.x : width - 1 - p.x) + 0.5) * 100) / width,
			y: (((orientation === 'white' ? height - 1 - p.y : p.y) + 0.5) * 100) / height
		};
	}
</script>

{#if lines.some((line) => line.path.length)}
	<svg class="search-overlay" viewBox="0 0 100 100" aria-label="Suggested moves" role="img">
		<defs
			>{#each colors as color, i}<marker
					id={`search-arrow-${i}`}
					markerWidth="4"
					markerHeight="4"
					refX="2.4"
					refY="2"
					orient="auto"><path d="M0 0 3 2 0 4Z" fill={color} /></marker
				>{/each}</defs
		>
		{#each lines as line, rank}
			{#if line.path.length}
				{@const end = point(line.path[line.path.length - 1])}
				<g class="candidate-arrow" data-rank={rank + 1} opacity={selected === rank ? 1 : 0.7}>
					{#if line.path.length === 1}
						<circle
							cx={point(line.path[0]).x}
							cy={point(line.path[0]).y}
							r={32 / width}
							fill="none"
							stroke={colors[rank]}
							stroke-width={selected === rank ? 1.2 : 0.9}
						/>
					{:else}
						{#each line.path.slice(1) as to, i}<line
								x1={point(line.path[i]).x}
								y1={point(line.path[i]).y}
								x2={point(to).x}
								y2={point(to).y}
								stroke={colors[rank]}
								stroke-width={selected === rank ? 1.5 : 1.1}
								stroke-linecap="round"
								marker-end={`url(#search-arrow-${rank})`}
							/>{/each}
					{/if}
					<circle
						cx={end.x + 25 / width}
						cy={end.y - 25 / height}
						r={16 / width}
						fill={colors[rank]}
						stroke="#24231f"
						stroke-width=".25"
					/>
					<text
						x={end.x + 25 / width}
						y={end.y - 25 / height}
						fill="#181b16"
						font-size={21 / width}
						font-weight="700"
						text-anchor="middle"
						dominant-baseline="central">{rank + 1}</text
					>
				</g>
			{/if}
		{/each}
	</svg>
{/if}
