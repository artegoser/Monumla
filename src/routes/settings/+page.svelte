<script lang="ts">
	import { resolve } from '$app/paths';
	import { enhance } from '$app/forms';
	import { countries } from '$lib/platform/catalog';
	let { data, form }: { data: import('./$types').PageData; form: import('./$types').ActionData } =
		$props();
	let submitting = $state(false);
</script>

<svelte:head><title>Edit profile · Monumla</title></svelte:head>
<div class="platform-page narrow-page">
	<div class="page-intro">
		<div>
			<h1>Your profile</h1>
			<p>A little about the person behind the moves.</p>
		</div>
		<a href={resolve(`/u/${data.profile.username}`)}>View profile ↗</a>
	</div>
	<section class="form-sheet settings-sheet">
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
			<label>Username<input value={data.profile.username} disabled /></label><label for="country"
				>Country<select id="country" name="country" value={form?.country ?? data.profile.country}
					><option value="">Prefer not to say</option
					>{#each countries as country (country.code)}<option value={country.code}
							>{country.name}</option
						>{/each}</select
				></label
			><label for="bio"
				>About you<textarea
					id="bio"
					name="bio"
					rows="6"
					maxlength="500"
					placeholder="Favourite games, a little about you…"
					>{form?.bio ?? data.profile.bio}</textarea
				><small>Up to 500 characters. Visible on your public profile.</small></label
			>{#if form?.message}<p
					class:form-success={form.saved}
					class:form-error={!form.saved}
					role="status"
				>
					{form.message}
				</p>{/if}<button class="button primary" disabled={submitting}
				>{submitting ? 'Saving…' : 'Save profile'}</button
			>
		</form>
	</section>
	<form method="POST" action="/logout" class="signout-form">
		<button class="text-button">Sign out of Monumla</button>
	</form>
</div>
