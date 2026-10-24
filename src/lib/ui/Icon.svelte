<script lang="ts">
	export let name:
		| 'first'
		| 'back'
		| 'next'
		| 'last'
		| 'flip'
		| 'settings'
		| 'play'
		| 'pause'
		| 'bot'
		| 'user'
		| 'reset'
		| 'rotate';
	const paths = {
		first: 'M5 5v14M19 5l-9 7 9 7V5Z',
		back: 'm15 5-8 7 8 7',
		next: 'm9 5 8 7-8 7',
		last: 'M19 5v14M5 5l9 7-9 7V5Z',
		flip: 'M4 8h15m-4-4 4 4-4 4M20 16H5m4-4-4 4 4 4',
		settings: 'M4 7h16M4 17h16M8 4v6M16 14v6',
		play: 'm8 4 12 8-12 8V4Z',
		pause: 'M8 5v14M16 5v14',
		bot: 'M5 8h14v12H5V8Zm7-5v5M9 13h.01M15 13h.01M9 17h6M2 12v4M22 12v4',
		user: 'M16 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0ZM4 21v-3a8 8 0 0 1 16 0v3',
		rotate: 'M20 9a8 8 0 1 0 0 6M20 3v6h-6',
		reset: 'M4 10a8 8 0 1 1 1 8M4 4v6h6'
	};
</script>

<svg
	width="20"
	height="20"
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width="1.7"
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden="true"><path d={paths[name]} /></svg
>
