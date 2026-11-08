<script lang="ts">
	import { resolve } from '$app/paths';
	import GameHistory from '$lib/platform/GameHistory.svelte';
	let { data }: { data: import('./$types').PageData } = $props();
</script>

<svelte:head><title>{data.own ? 'Your games' : 'Game history'} · Monumla</title></svelte:head>
<div class="platform-page content-page">
	<div class="page-intro">
		<div>
			<h1>{data.own ? 'Your games.' : 'Across the boards.'}</h1>
			<p>
				{data.own
					? 'Every opponent. Every lesson. Every move.'
					: 'Finished games from the Monumla community.'}
			</p>
		</div>
		{#if data.user}<a href={resolve(data.own ? '/games?all=1' : '/games')}
				>{data.own ? 'Explore all games ↗' : 'Your games ↗'}</a
			>{/if}
	</div>
	<GameHistory games={data.history.games} userId={data.user?.id} />{#if data.history.total > 20}<nav
			class="pagination"
			aria-label="History pages"
		>
			{#if data.history.page > 1}<a
					href={resolve(`/games?page=${data.history.page - 1}${data.own ? '' : '&all=1'}`)}
					>← Previous</a
				>{/if}<span>Page {data.history.page}</span
			>{#if data.history.page * 20 < data.history.total}<a
					href={resolve(`/games?page=${data.history.page + 1}${data.own ? '' : '&all=1'}`)}
					>Next →</a
				>{/if}
		</nav>{/if}
</div>
