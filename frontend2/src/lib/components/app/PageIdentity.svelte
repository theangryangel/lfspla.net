<script lang="ts">
	import type { Snippet } from 'svelte';
	let {
		label,
		children,
		highlight,
	}: { label: string; children: Snippet; highlight?: Snippet } = $props();
</script>

<header class="border-b bg-card text-card-foreground" aria-label={label}>
	<div
		class="mx-auto grid w-full max-w-screen-2xl items-center gap-x-8 gap-y-2 px-4 py-3 md:px-6 lg:grid-cols-[minmax(0,1fr)_auto] lg:py-4"
	>
		<div class="min-w-0">{@render children()}</div>
		{#if highlight}{@render highlight()}{/if}
	</div>
</header>
