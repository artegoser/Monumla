<script lang="ts">
	import Bg from '$lib/board/Bg.svelte';
	import Pieces from '$lib/board/Pieces.svelte';
	import AlignmentBoard from '$lib/games/AlignmentBoard.svelte';
	import BattleshipBoard from '$lib/games/BattleshipBoard.svelte';
	import type { GameView, InteractionView } from '$lib/wasm/runtime';
	let {
		game,
		interaction,
		orientation = 'white',
		side = 1,
		disabled = false,
		onchoose,
		oncancel
	}: {
		game: GameView;
		interaction: InteractionView;
		orientation?: 'white' | 'black';
		side?: number;
		disabled?: boolean;
		onchoose: (id: string) => void;
		oncancel: () => void;
	} = $props();
	const available = $derived(disabled ? { ...interaction, choices: [] } : interaction);
</script>

{#if game.ruleset === 'tictactoe'}<AlignmentBoard
		{game}
		interaction={available}
		{disabled}
		{onchoose}
	/>
{:else if game.ruleset === 'battleship'}<BattleshipBoard
		{game}
		interaction={available}
		viewer={side || 1}
		{disabled}
		{onchoose}
	/>
{:else}<div
		class="board-frame"
		class:go-frame={game.board_style === 'go'}
		class:orientation-black={orientation === 'black'}
	>
		<div class="board">
			<Bg
				width={game.width}
				height={game.height}
				{orientation}
				boardStyle={game.board_style}
			/><Pieces {game} interaction={available} {orientation} onchoice={onchoose} {oncancel} />
		</div>
	</div>{/if}
