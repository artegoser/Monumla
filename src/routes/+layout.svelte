<script lang="ts">
	import { resolve, asset } from '$app/paths';
	import '../app.css';
	import '../platform.css';
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { goto, invalidateAll } from '$app/navigation';
	import {
		connect,
		disconnect,
		connection,
		livePersonal,
		onRealtime,
		request
	} from '$lib/platform/realtime';
	import { gameName, timeLabel } from '$lib/platform/catalog';
	import Avatar from '$lib/platform/Avatar.svelte';
	let {
		data,
		children
	}: { data: import('./$types').LayoutData; children: import('svelte').Snippet } = $props();
	let mounted = $state(false);
	let message = $state('');
	const personal = $derived($livePersonal ?? data.personal);
	const incoming = $derived(personal?.challenges.filter((challenge) => challenge.incoming) ?? []);
	const requests = $derived(
		personal?.friends.filter((friend) => friend.incoming && friend.status === 'pending').length ?? 0
	);
	onMount(() => {
		mounted = true;
		const offMatch = onRealtime<{ id: string }>('match', (game) => {
			void goto(resolve(`/game/${game.id}`));
		});
		const offRatings = onRealtime('ratings.changed', () => {
			void invalidateAll();
		});
		return () => {
			offMatch();
			offRatings();
			disconnect();
		};
	});
	$effect(() => {
		if (mounted) connect(data.user?.id ?? null);
	});
	async function challengeAction(id: string, action: string) {
		try {
			await request(`challenge.${action}`, { challengeId: id });
			message = '';
		} catch (cause) {
			message = cause instanceof Error ? cause.message : String(cause);
		}
	}
</script>

<a class="skip-link" href="#main">Skip to content</a>
<header class="site-header">
	<a class="site-brand" href={resolve('/')} aria-label="Monumla home"
		><svg viewBox="0 0 32 32" fill="none" aria-hidden="true"
			><path
				d="M4 26V6l12 13L28 6v20M4 6l12 7L28 6"
				stroke="currentColor"
				stroke-width="2.5"
				stroke-linejoin="round"
			/></svg
		>monumla<span class="brand-period">.</span></a
	>
	<nav class="site-nav" aria-label="Main navigation">
		<a
			class:active={page.url.pathname === '/' || page.url.pathname.startsWith('/game/')}
			href={resolve('/')}>Play</a
		><a class:active={page.url.pathname === '/players'} href={resolve('/players')}>Players</a><a
			class:active={page.url.pathname === '/leaderboard'}
			href={resolve('/leaderboard')}>Rankings</a
		><a class:active={page.url.pathname === '/games'} href={resolve('/games')}
			>{data.user ? 'My games' : 'Games'}</a
		>
	</nav>
	<div class="account-nav">
		{#if data.user}<a class="friends-link" href={resolve('/friends')}
				>Friends{#if requests}<span class="notification-count">{requests}</span>{/if}</a
			><a class="account-link" href={resolve(`/u/${data.user.username}`)}
				><Avatar name={data.user.username} /><span>{data.user.username}</span></a
			>{:else}<a class="sign-in-link" href={resolve('/login')}>Sign in</a><a
				class="button small primary"
				href={resolve('/register')}>Join Monumla</a
			>{/if}
	</div>
</header>
{#if incoming.length}<div class="challenge-banner" aria-live="polite">
		{#each incoming as challenge (challenge.id)}<div>
				<p>
					<a href={resolve(`/u/${challenge.user.username}`)}>{challenge.user.username}</a>
					challenges you to {gameName(challenge.ruleset)}
					<span
						>· {timeLabel(challenge.initialSeconds, challenge.increment)} · {challenge.rated
							? 'Rated'
							: 'Casual'}</span
					>
				</p>
				<button class="button small primary" onclick={() => challengeAction(challenge.id, 'accept')}
					>Accept</button
				><button class="text-button" onclick={() => challengeAction(challenge.id, 'decline')}
					>Decline</button
				>
			</div>{/each}{#if message}<p class="form-error" role="alert">{message}</p>{/if}
	</div>{/if}
<main id="main">{@render children()}</main>
<footer class="site-footer">
	<span>Monumla · A place to play.</span>
	<nav aria-label="Footer">
		<a href={resolve('/analysis')}>Analysis</a><a href={asset('/source.tar.gz')}>Source code</a><a
			href={resolve('/license')}>AGPLv3</a
		><span class="connection-label"
			><span class="presence-dot" class:disconnected={$connection !== 'online'}
			></span>{$connection === 'online'
				? 'Connected'
				: $connection === 'connecting'
					? 'Connecting'
					: 'Reconnecting'}</span
		>
	</nav>
</footer>
