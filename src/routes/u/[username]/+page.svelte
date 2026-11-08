<script lang="ts">
	import { resolve } from '$app/paths';
	import { invalidateAll } from '$app/navigation';
	import { games, categories, countryName } from '$lib/platform/catalog';
	import { request, livePersonal, connection } from '$lib/platform/realtime';
	import Avatar from '$lib/platform/Avatar.svelte';
	import GameHistory from '$lib/platform/GameHistory.svelte';
	let { data }: { data: import('./$types').PageData } = $props();
	let error = $state('');
	let busy = $state(false);
	const self = $derived(data.user?.id === data.profile.id);
	const relationship = $derived(
		$livePersonal
			? $livePersonal.friends.find((friend) => friend.user.id === data.profile.id)
			: data.relationship
	);
	async function friendship() {
		busy = true;
		try {
			if (relationship?.status === 'accepted')
				await request('friend.remove', { friendshipId: relationship.id });
			else if (relationship?.incoming)
				await request('friend.accept', { friendshipId: relationship.id });
			else await request('friend.request', { userId: data.profile.id });
			error = '';
			await invalidateAll();
		} catch (cause) {
			error = cause instanceof Error ? cause.message : String(cause);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:head><title>{data.profile.username} · Monumla</title></svelte:head>
<div class="platform-page profile-page">
	<section class="profile-heading">
		<Avatar name={data.profile.username} large />
		<div class="profile-identity">
			<h1>{data.profile.username}</h1>
			<p>
				{countryName(data.profile.country)} <span>·</span> Joined {new Date(
					data.profile.createdAt
				).toLocaleDateString('en', { month: 'long', year: 'numeric' })}
			</p>
			{#if data.profile.bio}<p class="profile-bio">{data.profile.bio}</p>{:else if self}<a
					class="muted"
					href={resolve('/settings')}>Add a little about yourself →</a
				>{/if}
		</div>
		<div class="profile-actions">
			{#if self}<a class="button secondary" href={resolve('/settings')}>Edit profile</a
				>{:else if data.user}<a
					class="button primary"
					href={resolve(
						`/?opponent=${data.profile.id}&name=${encodeURIComponent(data.profile.username)}`
					)}>Challenge to a game</a
				><button
					class="button secondary"
					disabled={busy ||
						$connection !== 'online' ||
						(relationship?.status === 'pending' && !relationship.incoming)}
					onclick={friendship}
					>{relationship?.status === 'accepted'
						? 'Remove friend'
						: relationship?.incoming
							? 'Accept friend request'
							: relationship
								? 'Request sent'
								: 'Add friend'}</button
				>{:else}<a class="button primary" href={resolve('/login')}>Sign in to challenge</a>{/if}
		</div>
	</section>
	{#if error}<p class="form-error" role="alert">{error}</p>{/if}
	<section class="rating-section">
		<div class="section-heading">
			<h2>Ratings</h2>
			<span>Separate pools. Room to grow.</span>
		</div>
		<p class="rating-scroll-hint" id="rating-scroll-hint">
			Scroll horizontally to see Rapid and Classical ratings →
		</p>
		<div
			class="table-scroll"
			role="region"
			aria-label="Ratings by game and time control"
			aria-describedby="rating-scroll-hint"
		>
			<table class="rating-table">
				<thead
					><tr
						><th>Game</th>{#each categories as category (category)}<th>{category}</th>{/each}</tr
					></thead
				><tbody
					>{#each games as game (game.id)}<tr
							><th>{game.name}</th>{#each categories as category (category)}{@const rating =
									data.ratings.find(
										(item) => item.ruleset === game.id && item.category === category
									)}<td
									><a href={resolve(`/leaderboard?game=${game.id}&mode=${category}`)}
										><strong
											>{rating?.value ?? 1500}{!rating || rating.games < 30 ? '?' : ''}</strong
										><small>{rating?.games ?? 0} games</small></a
									></td
								>{/each}</tr
						>{/each}</tbody
				>
			</table>
		</div>
		<p class="table-note">
			Ratings start at 1500. A question mark means provisional: fewer than 30 rated games. Computer
			and casual games do not change ratings.
		</p>
	</section>
	<section class="profile-history">
		<div class="section-heading">
			<h2>Game history <span class="count-label">{data.history.total}</span></h2>
			<span>Every game, every move.</span>
		</div>
		<GameHistory
			games={data.history.games}
			userId={data.profile.id}
		/>{#if data.history.total > 20}<nav class="pagination" aria-label="History pages">
				{#if data.history.page > 1}<a
						href={resolve(`/u/${data.profile.username}?page=${data.history.page - 1}`)}
						>← Previous</a
					>{/if}<span>Page {data.history.page}</span
				>{#if data.history.page * 20 < data.history.total}<a
						href={resolve(`/u/${data.profile.username}?page=${data.history.page + 1}`)}>Next →</a
					>{/if}
			</nav>{/if}
	</section>
</div>
