<script lang="ts">
	import { resolve } from '$app/paths';
	import { enhance } from '$app/forms';
	import { countries } from '$lib/platform/catalog';
	let { form }: { form: import('./$types').ActionData } = $props();
	let submitting = $state(false);
</script>

<svelte:head><title>Join Monumla</title></svelte:head>
<div class="auth-layout">
	<div class="auth-story">
		<h1>One account.<br />Every game.</h1>
		<p>
			Find your people across the board. Build your ratings, challenge a friend, and keep every
			game.
		</p>
		<div class="auth-board-pattern" aria-hidden="true">
			{#each Array(64).keys() as i (i)}<span class:filled={(i + Math.floor(i / 8)) % 2 === 0}
				></span>{/each}<img src="/pieces/W/Knight.svg" alt="" />
		</div>
	</div>
	<section class="form-sheet">
		<h2>Create your account</h2>
		<p class="muted">Already a member? <a href={resolve('/login')}>Sign in</a></p>
		<form
			method="POST"
			use:enhance={() => {
				submitting = true;
				return async ({ update }) => {
					await update();
					submitting = false;
				};
			}}
		>
			{#if form?.message}<p class="form-error" role="alert">{form.message}</p>{/if}<label
				for="username"
				>Username<input
					id="username"
					name="username"
					autocomplete="username"
					required
					minlength="3"
					maxlength="24"
					value={form?.username ?? ''}
				/><small>3–24 letters, numbers, underscores, or hyphens.</small></label
			><label for="email"
				>Email<input
					id="email"
					name="email"
					type="email"
					autocomplete="email"
					required
					maxlength="254"
					value={form?.email ?? ''}
				/><small>Your email is private.</small></label
			><label for="password"
				>Password<input
					id="password"
					name="password"
					type="password"
					autocomplete="new-password"
					required
					minlength="10"
					maxlength="128"
				/><small>At least 10 characters.</small></label
			><label for="country"
				>Country <span class="muted">(optional)</span><select
					id="country"
					name="country"
					value={form?.country ?? ''}
					><option value="">Prefer not to say</option
					>{#each countries as country (country.code)}<option value={country.code}
							>{country.name}</option
						>{/each}</select
				></label
			><button class="button primary" disabled={submitting}
				>{submitting ? 'Creating your account…' : 'Create account'}</button
			>
		</form>
		<p class="form-note">Free to play. Open source. A place to get better.</p>
	</section>
</div>
