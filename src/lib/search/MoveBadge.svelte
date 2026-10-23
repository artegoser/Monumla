<script lang="ts">
	import type { MoveAssessment } from './types';
	import { describeAssessment } from './assessment';
	export let assessment: MoveAssessment;
	export let title = '';
	$: grade = describeAssessment(assessment);
</script>

<span
	class="move-badge"
	data-quality={grade.tone}
	style:--quality-color={grade.color}
	title={title || grade.name}
	aria-label={grade.name}
>
	{#if assessment.symbol === '✓'}
		<svg viewBox="0 0 20 20" aria-hidden="true"
			><path
				d="m4 10 4 4 8-9"
				fill="none"
				stroke="currentColor"
				stroke-width="2.3"
				stroke-linecap="round"
				stroke-linejoin="round"
			/></svg
		>
	{:else if assessment.symbol === '·'}
		<svg viewBox="0 0 20 20" aria-hidden="true"
			><circle cx="10" cy="10" r="3" fill="currentColor" /></svg
		>
	{:else}<span aria-hidden="true">{assessment.symbol ?? '—'}</span>{/if}
</span>
