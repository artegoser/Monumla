<script lang="ts">
	import { onMount } from 'svelte';
	import { SearchClient, SearchCancelled } from './client';
	import type { MoveAssessment, AnalysisView } from './types';
	import type { LocalGame, HistoryTurnView } from '$lib/wasm/runtime';
	export let runtime: LocalGame;
	export let revision: number;
	export let history: HistoryTurnView[];
	export let keys: string[];
	export let beforeKeys: string[];
	export let analyses: Record<string, AnalysisView>;
	export let results: Record<string, MoveAssessment>;
	export let onassessment: (key: string, value: MoveAssessment) => void;
	const engine = new SearchClient();
	let mounted = false;
	let reviewing = false;
	let paused = false;
	let failure = '';
	let seen = '';
	let token = 0;
	let currentMove = 0;
	$: completed = keys.filter((key) => results[key] != null).length;
	$: schedule(mounted, revision, paused);
	function schedule(ready: boolean, version: number, suspended: boolean) {
		if (!ready) return;
		const key = `${version}:${suspended}`;
		if (seen === key) return;
		seen = key;
		token++;
		engine.cancel();
		reviewing = false;
		if (!suspended) void review(token);
	}
	async function review(request: number) {
		if (!history.length) return;
		reviewing = true;
		failure = '';
		try {
			for (let i = history.length - 1; i >= 0; i--) {
				if (request !== token) return;
				const cached = analyses[beforeKeys[i]];
				if (results[keys[i]] && (!cached || cached.depth <= results[keys[i]].depth)) continue;
				currentMove = i + 1;
				const key = keys[i];
				const retained = cached ? runtime.reviewCachedTurn(history[i].index, cached) : null;
				const result =
					retained ??
					(await engine.review(runtime.reviewSnapshot(history[i].index), history[i].index));
				if (request !== token) return;
				onassessment(key, result);
			}
		} catch (cause) {
			if (request !== token || cause instanceof SearchCancelled) return;
			failure = cause instanceof Error ? cause.message : String(cause);
		} finally {
			if (request === token) reviewing = false;
		}
	}
	onMount(() => {
		mounted = true;
		return () => {
			token++;
			engine.dispose();
		};
	});
</script>

{#if history.length}
	<div class="review-status" aria-label="Game review" aria-live="polite">
		<span
			>{reviewing ? `Reviewing move ${currentMove}` : paused ? 'Review paused' : 'Game review'}
			<small>{completed}/{history.length}</small></span
		>
		{#if reviewing || paused}<button type="button" on:click={() => (paused = !paused)}
				>{paused ? 'Resume review' : 'Pause review'}</button
			>{/if}
	</div>
	{#if failure}<p class="engine-error" role="alert">{failure}</p>{/if}
{/if}
