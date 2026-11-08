<script lang="ts">
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';
	import { invalidateAll } from '$app/navigation';
	import { connection, onRealtime, request } from '$lib/platform/realtime';
	import { gameName, timeLabel, resultExplanation } from '$lib/platform/catalog';
	import type { LiveGame } from '$lib/platform/types';
	import type { InteractionView } from '$lib/wasm/runtime';
	import MatchBoard from '$lib/platform/MatchBoard.svelte';
	import Avatar from '$lib/platform/Avatar.svelte';
	import GameIcon from '$lib/platform/GameIcon.svelte';
	let { data }: { data: import('./$types').PageData } = $props();
	let live = $state<LiveGame | null>(null);
	let error = $state('');
	let busy = $state(false);
	let now = $state(0);
	let receivedAt = $state(0);
	let replay = $state<number | null>(null);
	let flipped = $state(false);
	let confirmResign = $state(false);
	const empty: InteractionView = {
		generation: '',
		selected_entity: null,
		pending_target: null,
		active_ability: null,
		choices: []
	};
	const metadata = $derived(live?.id === data.game.id ? live : null);
	const finished = $derived(metadata?.status === 'finished' || data.game.status === 'finished');
	const side = $derived(metadata?.side ?? data.side);
	const canFlip = $derived(!['tictactoe', 'battleship'].includes(data.game.ruleset));
	const orientation = $derived((side === 2) !== (canFlip && flipped) ? 'black' : 'white');
	const bottom = $derived(orientation === 'white' ? 1 : 2);
	const top = $derived(bottom === 1 ? 2 : 1);
	const board = $derived(
		replay !== null
			? data.moves[replay]?.view
			: (metadata?.view ?? data.moves[data.moves.length - 1]?.view)
	);
	const interaction = $derived(replay === null && metadata ? metadata.interaction : empty);
	const options = $derived(
		interaction.choices.filter(
			(choice) =>
				['select_option', 'select_ability', 'finish_turn'].includes(choice.kind) &&
				!choice.option_entity_type
		)
	);
	const history = $derived(
		metadata?.history ??
			data.moves
				.filter((move) => move.ply > 0)
				.map((move) => ({ index: move.ply - 1, actor: move.actor, notation: move.notation }))
	);
	function username(player: number) {
		return (
			(player === 1 ? data.game.firstUser : data.game.secondUser)?.username ??
			`Monumla · Level ${data.game.computerLevel}`
		);
	}
	function rating(player: number) {
		if (!(player === 1 ? data.game.firstUser : data.game.secondUser)) return 'Computer';
		return player === 1 ? data.game.firstRating : data.game.secondRating;
	}
	function clock(player: number) {
		if (!metadata) return '--:--';
		const ms = Math.max(
			0,
			(player === 1 ? metadata.firstMs : metadata.secondMs) -
				(!finished && metadata.activeSide === player ? Math.max(0, now - receivedAt) : 0)
		);
		const total = Math.ceil(ms / 1000);
		return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, '0')}`;
	}
	function delta(player: number) {
		const value =
			player === 1
				? (metadata?.firstDelta ?? data.game.firstDelta)
				: (metadata?.secondDelta ?? data.game.secondDelta);
		return value == null ? '' : value > 0 ? `+${value}` : String(value);
	}
	async function watch(id: string) {
		try {
			const state = await request<LiveGame>('game.watch', { gameId: id });
			if (data.game.id === id) {
				live = state;
				receivedAt = now = performance.now();
				error = '';
			}
		} catch (cause) {
			error = cause instanceof Error ? cause.message : String(cause);
		}
	}
	onMount(() => {
		const interval = setInterval(() => {
			now = performance.now();
		}, 100);
		const off = onRealtime<LiveGame>('game', (state) => {
			if (state.id !== data.game.id) return;
			const justFinished = live?.status === 'active' && state.status === 'finished';
			live = state;
			receivedAt = now = performance.now();
			if (justFinished) void invalidateAll();
		});
		return () => {
			clearInterval(interval);
			off();
			void request('game.unwatch').catch(() => {});
		};
	});
	$effect(() => {
		const id = data.game.id;
		if ($connection === 'online') {
			replay = null;
			void watch(id);
		}
	});
	async function act(action: string, fields: Record<string, unknown> = {}) {
		if (!metadata || busy) return;
		busy = true;
		error = '';
		try {
			await request(`game.${action}`, {
				gameId: metadata.id,
				revision: metadata.revision,
				...fields
			});
		} catch (cause) {
			error = cause instanceof Error ? cause.message : String(cause);
		} finally {
			busy = false;
			confirmResign = false;
		}
	}
	function choose(id: string) {
		void act('choose', { choice: id });
	}
	function seek(ply: number) {
		replay = Math.max(0, Math.min(data.moves.length - 1, ply));
	}
</script>

<svelte:head
	><title>{gameName(data.game.ruleset)} · {username(1)} vs {username(2)} · Monumla</title
	></svelte:head
>
<div class="platform-page match-page">
	<div class="match-breadcrumb">
		<a href={resolve('/')}>← Play</a><span
			>{gameName(data.game.ruleset)} · {timeLabel(data.game.initialSeconds, data.game.increment)} · {data
				.game.category} · {data.game.rated ? 'Rated' : 'Casual'}</span
		><span class="match-spectator">{side ? `You are player ${side}` : 'Spectating'}</span>
	</div>
	<div class="online-match-layout">
		<div class="online-board-column">
			<div class="match-player" class:has-turn={!finished && metadata?.activeSide === top}>
				<Avatar name={username(top)} />
				<div>
					{#if top === 1 ? data.game.firstUser : data.game.secondUser}<a
							href={resolve(`/u/${username(top)}`)}>{username(top)}</a
						>{:else}<strong>{username(top)}</strong>{/if}<small
						>{rating(top)}
						{data.game.rated ? 'Elo' : '· casual'}
						<span class="rating-delta">{delta(top)}</span></small
					>
				</div>
				<span class="game-clock" class:clock-running={!finished && metadata?.activeSide === top}
					>{clock(top)}</span
				>
			</div>
			{#if board}<MatchBoard
					game={board}
					{interaction}
					{side}
					{orientation}
					disabled={busy || !metadata || $connection !== 'online' || replay !== null || finished}
					onchoose={choose}
					oncancel={() => void act('cancel')}
				/>{:else}<div class="board-loading">Loading your board…</div>{/if}
			<div class="match-player" class:has-turn={!finished && metadata?.activeSide === bottom}>
				<Avatar name={username(bottom)} />
				<div>
					{#if bottom === 1 ? data.game.firstUser : data.game.secondUser}<a
							href={resolve(`/u/${username(bottom)}`)}>{username(bottom)}</a
						>{:else}<strong>{username(bottom)}</strong>{/if}<small
						>{rating(bottom)}
						{data.game.rated ? 'Elo' : '· casual'}
						<span class="rating-delta">{delta(bottom)}</span></small
					>
				</div>
				<span class="game-clock" class:clock-running={!finished && metadata?.activeSide === bottom}
					>{clock(bottom)}</span
				>
			</div>
			<div class="board-underbar">
				<span
					>{replay !== null
						? `Reviewing turn ${data.moves[replay]?.ply}`
						: finished
							? 'Game finished'
							: metadata?.activeSide === side
								? 'Your turn'
								: 'Waiting for your opponent'}</span
				>{#if canFlip}<button
						class="text-button"
						onclick={() => {
							flipped = !flipped;
						}}>Flip board</button
					>{/if}
			</div>
		</div>
		<aside class="match-sidebar">
			<section class="match-status">
				<GameIcon kind={finished ? 'trophy' : 'clock'} size={26} />
				<div>
					<h1>
						{finished
							? (metadata?.result ?? data.game.result ?? 'Game finished')
							: $connection !== 'online'
								? 'Reconnecting…'
								: metadata?.activeSide === side
									? 'Your move.'
									: side
										? 'Opponent’s move.'
										: 'Live game'}
					</h1>
					<p>
						{finished
							? resultExplanation(metadata?.termination ?? data.game.termination)
							: $connection !== 'online'
								? 'Your game and clock continue on the server.'
								: (board?.status.text ?? 'Connecting to your game…')}
					</p>
				</div>
			</section>
			{#if error}<p class="form-error match-error" role="alert">
					{error}
				</p>{/if}{#if metadata?.drawOffer && !finished}<div class="draw-offer">
					<p>
						{metadata.drawOffer === side ? 'You offered a draw.' : 'Your opponent offers a draw.'}
					</p>
					{#if metadata.drawOffer !== side && side}<button
							class="button small primary"
							disabled={busy}
							onclick={() => void act('draw.accept')}>Accept draw</button
						><button class="text-button" disabled={busy} onclick={() => void act('draw.decline')}
							>Decline</button
						>{/if}
				</div>{/if}
			<div class="match-notations" aria-label="Move history">
				{#each history as move, i (move.index)}<button
						class:current={replay === i + 1}
						disabled={!finished || i + 1 >= data.moves.length}
						onclick={() => seek(i + 1)}
						><span>{i + 1}.</span><span>{move.notation}</span><small>P{move.actor}</small></button
					>{:else}<p class="muted">The first move is yours to make.</p>{/each}
			</div>
			{#if finished}<div class="replay-controls" role="group" aria-label="Review controls">
					<button aria-label="Starting position" onclick={() => seek(0)}>⏮</button><button
						aria-label="Previous turn"
						onclick={() => seek((replay ?? data.moves.length - 1) - 1)}>‹</button
					><button
						aria-label="Next turn"
						onclick={() => seek((replay ?? data.moves.length - 1) + 1)}>›</button
					><button
						aria-label="Final position"
						onclick={() => {
							replay = null;
						}}>⏭</button
					>
				</div>
				<div class="finished-actions">
					<a class="button primary" href={resolve('/')}>Play another game</a
					>{#if data.game.ruleset === 'chess'}<a
							class="button secondary"
							href={resolve(`/game/${data.game.id}/pgn`)}
							download>Download PGN</a
						>{/if}<a href={resolve('/games')}>View game history →</a>
				</div>{:else if side}<div class="match-game-actions">
					{#if options.length}<div class="runtime-options">
							{#each options as option (option.id)}<button
									class="button small secondary"
									disabled={busy || $connection !== 'online'}
									onclick={() => choose(option.id)}
									>{option.label ?? option.option_key ?? option.kind.replaceAll('_', ' ')}</button
								>{/each}
						</div>{/if}{#if confirmResign}<p class="muted">Resign this game?</p>
						<div class="row-actions">
							<button
								class="button small secondary"
								onclick={() => {
									confirmResign = false;
								}}>Keep playing</button
							><button
								class="button small danger"
								disabled={busy}
								onclick={() => void act('resign')}>Confirm resignation</button
							>
						</div>{:else}<div class="match-action-row">
							{#if !data.game.computerLevel}<button
									class="text-button"
									disabled={busy || $connection !== 'online' || !!metadata?.drawOffer}
									onclick={() => void act('draw')}>Offer draw</button
								>{/if}<button
								class="text-button resign-button"
								disabled={busy || $connection !== 'online'}
								onclick={() => {
									confirmResign = true;
								}}>Resign</button
							>
						</div>{/if}
				</div>{/if}
			<p class="match-save-note">
				Every completed turn is saved. Reconnect to return to your game.
			</p>
		</aside>
	</div>
</div>
