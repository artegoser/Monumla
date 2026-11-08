<script lang="ts">
	import { resolve } from '$app/paths';
	import { livePersonal, request, connection } from '$lib/platform/realtime';
	import { gameName, timeLabel, countryName } from '$lib/platform/catalog';
	import Avatar from '$lib/platform/Avatar.svelte';
	import GameIcon from '$lib/platform/GameIcon.svelte';
	let { data }: { data: import('./$types').PageData } = $props();
	let error = $state('');
	const personal = $derived($livePersonal ?? data.personal);
	async function action(type: string, id: string, challenge = false) {
		try {
			await request(type, challenge ? { challengeId: id } : { friendshipId: id });
			error = '';
		} catch (cause) {
			error = cause instanceof Error ? cause.message : String(cause);
		}
	}
</script>

<svelte:head><title>Friends · Monumla</title></svelte:head>
<div class="platform-page content-page">
	<div class="page-intro">
		<div>
			<h1>Familiar faces.</h1>
			<p>Your friends, your rivals, your next rematch.</p>
		</div>
		<a class="button primary" href={resolve('/players')}>Find players</a>
	</div>
	{#if error}<p class="form-error" role="alert">{error}</p>{/if}
	<section>
		<div class="section-heading">
			<h2>Friends & requests</h2>
			<span>{personal?.friends.length ?? 0} connections</span>
		</div>
		{#if personal?.friends.length}<div class="friend-list">
				{#each personal.friends as friend (friend.id)}<div class="friend-row">
						<a class="friend-identity" href={resolve(`/u/${friend.user.username}`)}
							><Avatar name={friend.user.username} /><span
								><strong>{friend.user.username}</strong><small
									>{friend.status === 'pending'
										? friend.incoming
											? 'Wants to be your friend'
											: 'Friend request sent'
										: countryName(friend.user.country)}</small
								></span
							></a
						>{#if $livePersonal?.online?.includes(friend.user.id)}<span class="online-label"
								><span class="presence-dot"></span>Online</span
							>{/if}
						<div class="row-actions">
							{#if friend.status === 'pending' && friend.incoming}<button
									class="button small primary"
									disabled={$connection !== 'online'}
									onclick={() => action('friend.accept', friend.id)}>Accept</button
								>{:else if friend.status === 'accepted'}<a
									class="button small secondary"
									href={resolve(
										`/?opponent=${friend.user.id}&name=${encodeURIComponent(friend.user.username)}`
									)}>Challenge</a
								>{/if}<button
								class="text-button"
								disabled={$connection !== 'online'}
								onclick={() => action('friend.remove', friend.id)}
								>{friend.status === 'accepted'
									? 'Remove'
									: friend.incoming
										? 'Decline'
										: 'Cancel'}</button
							>
						</div>
					</div>{/each}
			</div>{:else}<div class="empty-state">
				<GameIcon kind="people" size={40} />
				<h3>A good game starts with good company.</h3>
				<p>Search for a player and send your first friend request.</p>
				<a class="button secondary" href={resolve('/players')}>Find players</a>
			</div>{/if}
	</section>
	<section class="outgoing-challenges">
		<div class="section-heading">
			<h2>Pending challenges</h2>
			<span>Available for 10 minutes</span>
		</div>
		{#each personal?.challenges ?? [] as challenge (challenge.id)}<div class="friend-row">
				<div>
					<strong>{challenge.incoming ? 'From' : 'To'} {challenge.user.username}</strong>
					<p class="muted">
						{gameName(challenge.ruleset)} · {timeLabel(
							challenge.initialSeconds,
							challenge.increment
						)} · {challenge.rated ? 'Rated' : 'Casual'}
					</p>
				</div>
				<div class="row-actions">
					{#if challenge.incoming}<button
							class="button small primary"
							onclick={() => action('challenge.accept', challenge.id, true)}>Accept</button
						><button
							class="text-button"
							onclick={() => action('challenge.decline', challenge.id, true)}>Decline</button
						>{:else}<button
							class="text-button"
							onclick={() => action('challenge.cancel', challenge.id, true)}>Cancel</button
						>{/if}
				</div>
			</div>{:else}<p class="muted">No pending challenges.</p>{/each}
	</section>
</div>
