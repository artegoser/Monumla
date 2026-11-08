<script lang="ts">
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';
	import { request, connection, livePersonal } from '$lib/platform/realtime';
	import { countryName } from '$lib/platform/catalog';
	import type { PlayerResults } from '$lib/platform/types';
	import Avatar from '$lib/platform/Avatar.svelte';
	import GameIcon from '$lib/platform/GameIcon.svelte';
	let { data }: { data: import('./$types').PageData } = $props();
	let query = $state('');
	let players = $state<PlayerResults | null>(null);
	let error = $state('');
	let generation = 0;
	let timer: ReturnType<typeof setTimeout>;
	const results = $derived(players ?? data.players);
	onMount(() => {
		query = data.query;
		return () => clearTimeout(timer);
	});
	function search() {
		const current = ++generation;
		clearTimeout(timer);
		timer = setTimeout(async () => {
			try {
				const found = await request<PlayerResults>('users.search', { query });
				if (current === generation) {
					players = found;
					error = '';
				}
			} catch (cause) {
				error = cause instanceof Error ? cause.message : String(cause);
			}
		}, 200);
	}
</script>

<svelte:head><title>Find players · Monumla</title></svelte:head>
<div class="platform-page content-page">
	<div class="page-intro">
		<div>
			<h1>Meet your next opponent.</h1>
			<p>Find a familiar name. Make a new connection.</p>
		</div>
		<GameIcon kind="people" size={44} />
	</div>
	<form
		class="player-search"
		method="GET"
		role="search"
		onsubmit={(event) => {
			if ($connection === 'online') {
				event.preventDefault();
				search();
			}
		}}
	>
		<GameIcon kind="search" /><input
			aria-label="Search players"
			name="q"
			placeholder="Search players by username"
			bind:value={query}
			oninput={search}
			maxlength="30"
		/><button class="button secondary">Search</button>
	</form>
	{#if error}<p class="form-error" role="alert">{error}</p>{/if}
	<div class="player-directory">
		{#each results as player (player.id)}<a
				class="directory-player"
				href={resolve(`/u/${player.username}`)}
				><Avatar name={player.username} />
				<div><strong>{player.username}</strong><small>{countryName(player.country)}</small></div>
				{#if $livePersonal?.online?.includes(player.id)}<span class="online-label"
						><span class="presence-dot"></span>Online</span
					>{/if}<span class="directory-rating"
					>{player.ratings[0]?.value ?? '1500?'}<small
						>{player.ratings.length ? 'top rating' : 'unrated'}</small
					></span
				><span>›</span></a
			>{:else}<div class="empty-state">
				<GameIcon kind="search" size={38} />
				<h3>No players found</h3>
				<p>Try another username.</p>
			</div>{/each}
	</div>
</div>
