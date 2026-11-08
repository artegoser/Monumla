<script lang="ts">
	import { resolve } from '$app/paths';
	import { gameName, timeLabel } from './catalog';
	import type { GameSummary } from './types';
	import GameIcon from './GameIcon.svelte';
	let { games, userId }: { games: GameSummary[]; userId?: string } = $props();
	function opponent(game: GameSummary, first: boolean) {
		return (
			(first ? game.firstUser : game.secondUser)?.username ?? `Computer level ${game.computerLevel}`
		);
	}
</script>

{#if games.length}<div class="history-list">
		{#each games as game (game.id)}<a class="history-entry" href={resolve(`/game/${game.id}`)}
				><span class="history-game-icon"
					><GameIcon
						kind={game.ruleset.startsWith('checkers') ? 'checkers' : game.ruleset}
						size={30}
					/></span
				>
				<div class="history-players">
					<strong
						>{opponent(game, true)} <span class="muted">vs</span> {opponent(game, false)}</strong
					><span
						>{gameName(game.ruleset)} · {timeLabel(game.initialSeconds, game.increment)} · {game.category}
						· {game.rated ? 'Rated' : 'Casual'}</span
					>
				</div>
				<div
					class="history-result"
					class:win={!!userId &&
						((game.result === '1-0' && game.firstUser?.id === userId) ||
							(game.result === '0-1' && game.secondUser?.id === userId))}
				>
					<strong>{game.result ?? 'In progress'}</strong><small
						>{new Date(game.createdAt).toLocaleDateString('en', {
							month: 'short',
							day: 'numeric'
						})}</small
					>
				</div>
				<span class="muted">›</span></a
			>{/each}
	</div>{:else}<div class="empty-state">
		<GameIcon kind="history" size={38} />
		<h3>No games yet</h3>
		<p>Every game you play will be here, ready to revisit.</p>
		<a class="button secondary" href={resolve('/')}>Play your first game</a>
	</div>{/if}
