<script lang="ts">
	import { numToAlpha } from '$lib/utils';

	export let width: number;
	export let height: number;
	export let orientation: 'white' | 'black' = 'white';
	export let boardStyle: 'checkerboard' | 'go' | 'battleship' | 'tictactoe' = 'checkerboard';
	export let showCoordinates = true;

	function boardX(col: number) {
		return orientation === 'white' ? col : width - col - 1;
	}

	function boardY(row: number) {
		return orientation === 'white' ? height - row - 1 : row;
	}

	function goFile(x: number) {
		const code = 'A'.charCodeAt(0) + x + (x >= 8 ? 1 : 0);
		return String.fromCharCode(code);
	}

	function goStarPoints(size: number) {
		if (size === 19) {
			const axes = [3, 9, 15];
			return axes.flatMap((x) => axes.map((y) => ({ x, y })));
		}
		if (size === 13) {
			return [
				{ x: 3, y: 3 }, { x: 3, y: 9 },
				{ x: 9, y: 3 }, { x: 9, y: 9 },
				{ x: 6, y: 6 }
			];
		}
		if (size === 9) {
			return [
				{ x: 2, y: 2 }, { x: 2, y: 6 },
				{ x: 6, y: 2 }, { x: 6, y: 6 },
				{ x: 4, y: 4 }
			];
		}
		return size % 2 === 1 && size >= 5
			? [{ x: Math.floor(size / 2), y: Math.floor(size / 2) }]
			: [];
	}

	$: starPoints = boardStyle === 'go' && width === height ? goStarPoints(width) : [];
</script>

<div
	class="board-layer board-background"
	class:go-board={boardStyle === 'go'}
	style="--width: {width}; --height: {height};"
	aria-hidden="true"
>
	{#if boardStyle === 'go'}
		<svg class="go-grid-lines" viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none">
			{#each { length: width } as _, col}
				<line x1={col + 0.5} y1="0.5" x2={col + 0.5} y2={height - 0.5} />
			{/each}
			{#each { length: height } as _, row}
				<line x1="0.5" y1={row + 0.5} x2={width - 0.5} y2={row + 0.5} />
			{/each}
			{#each starPoints as point}
				<circle class="go-hoshi" cx={point.x + 0.5} cy={point.y + 0.5} r={Math.max(0.08, width / 180)} />
			{/each}
		</svg>
	{/if}

	{#each { length: height } as _, row}
		{#each { length: width } as _, col}
			{@const x = boardX(col)}
			{@const y = boardY(row)}
			<div class="board-square" class:light={boardStyle !== 'go' && (x + y) % 2 === 1} class:dark={boardStyle !== 'go' && (x + y) % 2 === 0}>
				{#if showCoordinates && col === 0}
					<span class="board-coordinate rank">{y + 1}</span>
				{/if}
				{#if showCoordinates && row === height - 1}
					<span class="board-coordinate file">{boardStyle === 'go' ? goFile(x) : numToAlpha(x)}</span>
				{/if}
			</div>
		{/each}
	{/each}
</div>
