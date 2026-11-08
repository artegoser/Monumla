<script lang="ts">
	import { resolve } from '$app/paths';
	import { goto } from '$app/navigation';
	import { games, timeControls, categoryFor, type PublicUser, type Ruleset } from './catalog';
	import { request, connection, livePersonal } from './realtime';
	import type { Personal, PlayerResults } from './types';
	import GameIcon from './GameIcon.svelte';
	let {
		user,
		initialPersonal,
		opponent = null
	}: {
		user: PublicUser | null;
		initialPersonal: Personal | null;
		opponent?: { id: string; username: string } | null;
	} = $props();
	let ruleset = $state<Ruleset>('chess');
	let tab = $derived(opponent ? 'challenge' : 'quick');
	let seconds = $state(300);
	let increment = $state(3);
	let rated = $state(true);
	let level = $state(2);
	let side = $state(1);
	let custom = $state(false);
	let customMinutes = $state(10);
	let customIncrement = $state(0);
	let busy = $state(false);
	let error = $state('');
	let notice = $state('');
	let query = $state('');
	let results = $state<PlayerResults>([]);
	let selectedOpponent = $derived<{ id: string; username: string } | null>(opponent);
	let searchGeneration = 0;
	const personal = $derived($livePersonal ?? initialPersonal);
	const selected = $derived(games.find((game) => game.id === ruleset)!);
	const effectiveSeconds = $derived(custom ? Number(customMinutes) * 60 : seconds);
	const effectiveIncrement = $derived(custom ? Number(customIncrement) : increment);
	async function search() {
		const generation = ++searchGeneration;
		if (query.length < 2) {
			results = [];
			return;
		}
		try {
			const found = await request<PlayerResults>('users.search', { query });
			if (generation === searchGeneration)
				results = found.filter((player) => player.id !== user?.id);
		} catch (cause) {
			error = String(cause);
		}
	}
	async function play() {
		if (!user) {
			await goto(resolve('/login'));
			return;
		}
		busy = true;
		error = '';
		notice = '';
		const options = {
			ruleset,
			initialSeconds: effectiveSeconds,
			increment: effectiveIncrement,
			rated
		};
		try {
			if (tab === 'computer') {
				const result = await request<{ gameId: string }>('computer.create', {
					...options,
					rated: false,
					level: Number(level),
					side: Number(side)
				});
				await goto(resolve(`/game/${result.gameId}`));
			} else if (tab === 'challenge') {
				if (!selectedOpponent) throw new Error('Choose a player to challenge.');
				await request('challenge.create', { ...options, userId: selectedOpponent.id });
				notice = `Challenge sent to ${selectedOpponent.username}. It is available for 10 minutes.`;
			} else await request('queue.join', options);
		} catch (cause) {
			error = cause instanceof Error ? cause.message : String(cause);
		} finally {
			busy = false;
		}
	}
	async function leave() {
		try {
			await request('queue.leave');
		} catch (cause) {
			error = String(cause);
		}
	}
</script>

<div class="play-workbench">
	<nav class="game-picker" aria-label="Choose a game">
		{#each games as game (game.id)}<button
				class:chosen={ruleset === game.id}
				onclick={() => {
					ruleset = game.id;
					error = '';
				}}
				aria-pressed={ruleset === game.id}
				><GameIcon kind={game.icon} size={30} /><span
					><strong>{game.name}</strong><small>{game.board}</small></span
				><span class="picker-arrow">›</span></button
			>{/each}
		<a class="analysis-link" href={resolve('/analysis')}>Open analysis board <span>↗</span></a>
	</nav>
	<section class="pairing-panel" aria-label="Set up a game">
		<div class="pairing-tabs" role="tablist" aria-label="Play mode">
			{#each [{ id: 'quick', label: 'Quick pairing' }, { id: 'computer', label: 'Computer' }, { id: 'challenge', label: 'Challenge' }] as item (item.id)}<button
					role="tab"
					aria-selected={tab === item.id}
					onclick={() => {
						tab = item.id;
						error = '';
						notice = '';
					}}>{item.label}</button
				>{/each}
		</div>
		<div class="pairing-body">
			{#if personal?.activeGame}<div class="active-game-notice">
					<GameIcon kind="chess" />
					<div>
						<strong>Your game is in progress</strong>
						<p>Your opponent and your clock are waiting.</p>
					</div>
					<a class="button primary" href={resolve(`/game/${personal.activeGame.id}`)}
						>Return to game</a
					>
				</div>
			{:else if personal?.queue}<div class="queue-state" role="status">
					<div class="search-orbit"><GameIcon kind="search" size={36} /></div>
					<h2>Finding your opponent</h2>
					<p>
						{games.find((game) => game.id === personal.queue?.ruleset)?.name} · {personal.queue
							.initialSeconds / 60} + {personal.queue.increment} · {personal.queue.rated
							? 'Rated'
							: 'Casual'}
					</p>
					<p class="muted">Stay on Monumla. Your game opens as soon as someone joins.</p>
					<button class="button secondary" onclick={leave}>Cancel search</button>
				</div>
			{:else}
				<div class="selected-game-heading">
					<div>
						<h2>{selected.name}</h2>
						<p>{selected.detail}</p>
					</div>
					<GameIcon kind={selected.icon} size={40} />
				</div>
				{#if tab === 'challenge'}
					<div class="opponent-search">
						<label for="opponent">Your opponent</label>{#if selectedOpponent}<div
								class="selected-opponent"
							>
								<a href={resolve(`/u/${selectedOpponent.username}`)}>{selectedOpponent.username}</a
								><button
									class="text-button"
									onclick={() => {
										selectedOpponent = null;
									}}>Change</button
								>
							</div>{:else}<input
								id="opponent"
								placeholder="Search by username"
								bind:value={query}
								oninput={search}
								autocomplete="off"
							/>{#if results.length}<ul class="opponent-results">
									{#each results as player (player.id)}<li>
											<button
												onclick={() => {
													selectedOpponent = player;
													results = [];
												}}>{player.username}<span>{player.country}</span></button
											>
										</li>{/each}
								</ul>{/if}{/if}
					</div>
				{/if}
				<div class="time-grid" aria-label="Time control">
					{#each timeControls as time (time.label)}<button
							class:chosen={!custom && seconds === time.seconds && increment === time.increment}
							aria-pressed={!custom && seconds === time.seconds && increment === time.increment}
							onclick={() => {
								custom = false;
								seconds = time.seconds;
								increment = time.increment;
							}}><span>{time.label}</span><small>{time.category}</small></button
						>{/each}
				</div>
				<div class="pairing-preferences">
					<label class="checkbox-field"
						><input type="checkbox" bind:checked={rated} disabled={tab === 'computer'} /><span
							>{tab === 'computer' ? 'Computer games are casual' : 'Rated game'}</span
						></label
					><button
						class="text-button"
						aria-expanded={custom}
						onclick={() => {
							custom = !custom;
						}}>Custom time</button
					>
				</div>
				{#if custom}<div class="form-row custom-time">
						<label
							>Minutes per side<input
								type="number"
								min="1"
								max="180"
								step="1"
								bind:value={customMinutes}
							/></label
						><label
							>Increment in seconds<input
								type="number"
								min="0"
								max="180"
								step="1"
								bind:value={customIncrement}
							/></label
						>
					</div>{/if}
				{#if tab === 'computer'}<div class="form-row computer-options">
						<label
							>Computer strength<select bind:value={level}
								><option value={1}>Level 1 · Beginner</option><option value={2}
									>Level 2 · Casual</option
								><option value={3}>Level 3 · Challenging</option><option value={4}
									>Level 4 · Strong</option
								></select
							></label
						><label
							>Your side<select bind:value={side}
								><option value={1}>First player</option><option value={2}>Second player</option
								></select
							></label
						>
					</div>{/if}
				{#if error}<p class="form-error" role="alert">{error}</p>{/if}
				{#if notice}<p class="form-success" role="status">{notice}</p>{/if}
				<button
					class="button primary play-button"
					disabled={busy || (!!user && $connection !== 'online')}
					onclick={play}
					>{busy
						? 'Setting up your game…'
						: !user
							? 'Sign in to play'
							: tab === 'computer'
								? 'Play with the computer'
								: tab === 'challenge'
									? 'Send challenge'
									: 'Find an opponent'}<GameIcon kind="arrow" size={21} /></button
				>
				<p class="pairing-footnote">
					{tab === 'computer'
						? 'Powered by the Monumla game engine. Saved to your history.'
						: `Your ${selected.short.toLowerCase()} ${categoryFor(effectiveSeconds, effectiveIncrement)} rating has its own pool.`}
				</p>
			{/if}
		</div>
	</section>
</div>
