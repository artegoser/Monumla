<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import MatchSetup from '$lib/platform/MatchSetup.svelte';
	import GameIcon from '$lib/platform/GameIcon.svelte';
	import { liveLobby } from '$lib/platform/realtime';
	import { gameName, timeLabel } from '$lib/platform/catalog';
	let { data }: { data: import('./$types').PageData } = $props();
	const lobby = $derived($liveLobby ?? data.lobby);
	const opponent = $derived(
		page.url.searchParams.get('opponent')
			? {
					id: page.url.searchParams.get('opponent')!,
					username: page.url.searchParams.get('name') ?? 'Selected player'
				}
			: null
	);
</script>

<svelte:head
	><title>Monumla · Your next move</title><meta
		name="description"
		content="Play chess, checkers, Go, Battleship, and tic-tac-toe. Find opponents, challenge friends, and follow your progress on Monumla."
	/></svelte:head
>
<div class="platform-page lobby-page">
	<div class="page-intro">
		<div>
			<h1>Find your next game.</h1>
			<p>Six games. A whole world of opponents.</p>
		</div>
		<div class="live-count">
			<span class="presence-dot"></span><strong>{lobby.online}</strong> players online<span
				class="stat-divider">/</span
			><strong>{lobby.playing}</strong> games in play
		</div>
	</div>
	<div class="lobby-columns">
		<MatchSetup user={data.user} initialPersonal={data.personal} {opponent} />
		<aside class="lobby-rail">
			<section class="rail-section">
				<h2>Around the boards</h2>
				{#if lobby.recent.length}<div class="live-games">
						{#each lobby.recent as game (game.id)}<a href={resolve(`/game/${game.id}`)}
								><GameIcon kind={game.ruleset.startsWith('checkers') ? 'checkers' : game.ruleset} />
								<div>
									<strong
										>{game.firstUser?.username ?? 'Computer'} <span>vs</span>
										{game.secondUser?.username ?? 'Computer'}</strong
									><small
										>{gameName(game.ruleset)} · {timeLabel(
											game.initialSeconds,
											game.increment
										)}</small
									>
								</div></a
							>{/each}
					</div>{:else}<div class="quiet-empty">
						<GameIcon kind="chess" size={38} />
						<p>The boards are waiting.<br />Start a game and make the first move.</p>
					</div>{/if}
			</section>
			<section class="rail-section community-note">
				<h2>A place for every player.</h2>
				<p>
					One account for all your games. Separate ratings for every game and time control. Every
					move kept in your history.
				</p>
				<a href={resolve('/leaderboard')}>Explore the rankings <span>↗</span></a>
			</section>
			<div class="community-counts">
				<div><strong>{lobby.players}</strong><span>registered players</span></div>
				<div><strong>{lobby.played}</strong><span>games played</span></div>
			</div>
		</aside>
	</div>
	<div class="lobby-bottom">
		<span><GameIcon kind="people" size={20} /> Better with a familiar opponent.</span><a
			href={resolve(data.user ? '/friends' : '/register')}
			>{data.user ? 'Find your friends' : 'Create your free account'} <span>→</span></a
		>
	</div>
</div>
