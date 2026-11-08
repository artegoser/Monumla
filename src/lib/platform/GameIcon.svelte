<script lang="ts">
	let { kind, size = 24 }: { kind: string; size?: number } = $props();
</script>

<svg
	width={size}
	height={size}
	viewBox="0 0 32 32"
	fill="none"
	stroke="currentColor"
	stroke-width="1.7"
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden="true"
>
	{#if kind === 'chess'}<path
			d="M9 27h15M11 27v-5h10v5M12 22l1-5h7l1 5M13 17c-6-1-6-7-2-10l4-3 5 4 3 6-7-2-3 5M15 4v5"
		/><circle cx="17" cy="9" r=".7" fill="currentColor" />
	{:else if kind === 'checkers'}<ellipse cx="16" cy="20" rx="11" ry="6" /><path
			d="M5 20v4c0 3 5 5 11 5s11-2 11-5v-4M9 14V9c0-3 5-5 11-5s9 2 9 5v4c0 3-4 5-9 5"
		/><ellipse cx="20" cy="9" rx="9" ry="5" />
	{:else if kind === 'go'}<path d="M5 5h22v22H5zM12 5v22M20 5v22M5 12h22M5 20h22" /><circle
			cx="12"
			cy="12"
			r="4.5"
			fill="currentColor"
		/><circle cx="20" cy="20" r="4.5" fill="var(--surface, #2b2e31)" />
	{:else if kind === 'battleship'}<path
			d="M3 19l6 7h15l5-7H3ZM10 19v-7h12v7M15 12V5l8 4h-8M3 29l4-2 5 2 5-2 5 2 6-2"
		/>
	{:else if kind === 'tictactoe'}<path
			d="M12 3v26M21 3v26M3 12h26M3 21h26M4 4l5 5M9 4 4 9M23 23l5 5M28 23l-5 5"
		/><circle cx="16.5" cy="16.5" r="3" />
	{:else if kind === 'people'}<circle cx="12" cy="10" r="4" /><path
			d="M3 27v-3a9 9 0 0 1 18 0v3M22 6a4 4 0 0 1 0 8M24 19c4 1 5 4 5 8"
		/>
	{:else if kind === 'trophy'}<path
			d="M10 4h12v9a6 6 0 0 1-12 0V4ZM10 7H5v4c0 4 3 5 6 5M22 7h5v4c0 4-3 5-6 5M16 19v7M10 28h12"
		/>
	{:else if kind === 'clock'}<circle cx="16" cy="16" r="12" /><path d="M16 8v9l6 3" />
	{:else if kind === 'history'}<path d="M5 12a12 12 0 1 1 0 9M5 4v8h8M16 9v8l5 3" />
	{:else if kind === 'arrow'}<path d="M5 16h22M19 8l8 8-8 8" />
	{:else if kind === 'search'}<circle cx="13" cy="13" r="8" /><path d="m19 19 9 9" />
	{:else if kind === 'computer'}<rect x="4" y="5" width="24" height="17" rx="2" /><path
			d="M16 22v6M10 28h12M10 12l3 3-3 3M17 18h5"
		/>
	{:else}<path d="M7 7h18v18H7zM12 12h8v8h-8z" />{/if}
</svg>
