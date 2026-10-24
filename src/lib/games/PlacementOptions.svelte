<script lang="ts">
	import { stateScalar, type ChoiceView } from '$lib/wasm/runtime';
	export let choices: ChoiceView[];
	export let onpreview: (choice: ChoiceView | null) => void;
	$: placements = choices.filter((c) => c.preview.length > 0).slice(0, 3);
	function coordinate(p: { x: number; y: number }) {
		return `${String.fromCharCode(65 + (p.x % 10))}${p.y + 1}`;
	}
</script>

<div class="placement-options">
	<p>Legal placement previews. Fleet positions are not strategically ranked.</p>
	{#each placements as choice}
		<button
			type="button"
			aria-label={`Preview ${choice.label} ${stateScalar(choice.data, 'vertical') ? 'vertical' : 'horizontal'} at ${coordinate(choice.preview[0])}`}
			on:mouseenter={() => onpreview(choice)}
			on:mouseleave={() => onpreview(null)}
			on:focus={() => onpreview(choice)}
			on:blur={() => onpreview(null)}
			on:click={() => onpreview(choice)}
		>
			<span class="ship-miniature" class:vertical={stateScalar(choice.data, 'vertical') === true}
				>{#each choice.preview as _}<i></i>{/each}</span
			>
			<span
				><strong
					>{coordinate(choice.preview[0])}–{coordinate(
						choice.preview[choice.preview.length - 1]
					)}</strong
				><small
					>{choice.preview.length} decks · {stateScalar(choice.data, 'vertical')
						? 'Vertical'
						: 'Horizontal'}</small
				></span
			>
		</button>
	{/each}
</div>

<style>
	.placement-options {
		padding-top: 8px;
	}
	.placement-options > p {
		color: var(--muted);
		font-size: 0.85rem;
		line-height: 1.5;
		margin: 0 0 14px;
	}
	.placement-options > button {
		display: flex;
		align-items: center;
		gap: 16px;
		width: 100%;
		min-height: 66px;
		border: 0;
		border-bottom: 1px solid var(--line);
		background: transparent;
		text-align: left;
		color: inherit;
		cursor: pointer;
	}
	.placement-options > button:hover,
	.placement-options > button:focus-visible {
		background: #353930;
	}
	.ship-miniature {
		display: flex;
		gap: 2px;
		width: 58px;
		justify-content: center;
		align-items: center;
	}
	.ship-miniature.vertical {
		flex-direction: column;
	}
	.ship-miniature i {
		display: block;
		width: 9px;
		height: 9px;
		background: #a8c784;
		border-radius: 1px;
	}
	.placement-options strong,
	.placement-options small {
		display: block;
	}
	.placement-options strong {
		font-weight: 500;
	}
	.placement-options small {
		margin-top: 5px;
		color: var(--muted);
		font-size: 0.8rem;
	}
</style>
