<script lang="ts">
	let { name, large = false }: { name: string; large?: boolean } = $props();
</script>

<span class="avatar" class:large aria-hidden="true">{name.slice(0, 2).toUpperCase()}</span>
