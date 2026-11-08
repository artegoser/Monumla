<script lang="ts">
	import { resolve } from '$app/paths';
	import { enhance } from '$app/forms';
	let { form }: { form: import('./$types').ActionData } = $props();
	let submitting = $state(false);
</script>

<svelte:head><title>Sign in · Monumla</title></svelte:head>
<div class="auth-layout login-layout">
	<div class="auth-story">
		<h1>Welcome back<br />to the board.</h1>
		<p>Your games, your friends, your next opponent. Pick up where you left off.</p>
		<div class="login-pieces" aria-hidden="true">
			<img src="/pieces/W/King.svg" alt="" /><img src="/pieces/B/Queen.svg" alt="" />
		</div>
	</div>
	<section class="form-sheet">
		<h2>Sign in</h2>
		<p class="muted">New to Monumla? <a href={resolve('/register')}>Create an account</a></p>
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
				for="identity"
				>Username or email<input
					id="identity"
					name="identity"
					autocomplete="username"
					required
					maxlength="254"
					value={form?.identity ?? ''}
				/></label
			><label for="password"
				>Password<input
					id="password"
					name="password"
					type="password"
					autocomplete="current-password"
					required
					maxlength="128"
				/></label
			><button class="button primary" disabled={submitting}
				>{submitting ? 'Signing in…' : 'Sign in'}</button
			>
		</form>
	</section>
</div>
