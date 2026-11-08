<script lang="ts">
	import { resolve } from '$app/paths';
	import { games, categories, gameName, countryName } from '$lib/platform/catalog';
	import GameIcon from '$lib/platform/GameIcon.svelte';
	import Avatar from '$lib/platform/Avatar.svelte';
	let { data }: { data: import('./$types').PageData } = $props();
</script>

<svelte:head><title>{gameName(data.ruleset)} rankings · Monumla</title></svelte:head>
<div class="platform-page content-page">
	<div class="page-intro">
		<div>
			<h1>The rankings.</h1>
			<p>A different board. A different climb.</p>
		</div>
		<GameIcon kind="trophy" size={44} />
	</div>
	<form class="ranking-filters" method="GET">
		<label
			>Game<select name="game" value={data.ruleset}
				>{#each games as game (game.id)}<option value={game.id}>{game.name}</option>{/each}</select
			></label
		><label
			>Time control<select name="mode" value={data.category}
				>{#each categories as category (category)}<option value={category}>{category}</option
					>{/each}</select
			></label
		><button class="button secondary">Show rankings</button>
	</form>
	<div class="rankings-heading">
		<h2>{gameName(data.ruleset)} <span>· {data.category}</span></h2>
		<span>Top 100 players</span>
	</div>
	{#if data.ratings.length}<div class="table-scroll">
			<table class="leaderboard-table">
				<thead
					><tr><th>Rank</th><th>Player</th><th>Country</th><th>Games</th><th>Elo</th></tr></thead
				><tbody
					>{#each data.ratings as rating, i (rating.id)}<tr
							><td class="rank-number">{i + 1}</td><td
								><a class="ranked-player" href={resolve(`/u/${rating.user.username}`)}
									><Avatar name={rating.user.username} />{rating.user.username}</a
								></td
							><td class="muted">{countryName(rating.user.country)}</td><td>{rating.games}</td><td
								class="elo-number">{rating.value}{rating.games < 30 ? '?' : ''}</td
							></tr
						>{/each}</tbody
				>
			</table>
		</div>{:else}<div class="empty-state">
			<GameIcon kind="trophy" size={42} />
			<h3>An open board.</h3>
			<p>
				No rated {gameName(data.ruleset).toLowerCase()}
				{data.category} games yet. Be the first to play.
			</p>
			<a class="button primary" href={resolve('/')}>Find an opponent</a>
		</div>{/if}
</div>
