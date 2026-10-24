<script lang="ts">
	import type { PositionView } from '$lib/wasm/runtime';
	import { arrowPath } from './arrow';
	export let from: PositionView;
	export let to: PositionView;
	export let orientation: 'white' | 'black';
	export let width: number;
	export let height: number;
	export let color: string;
	export let thickness = 1.5;
</script>

<path
	data-from={`${from.x},${from.y}`}
	data-to={`${to.x},${to.y}`}
	d={arrowPath(from, to, orientation, width, height, thickness)}
	fill={color}
/>
