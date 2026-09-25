<script lang="ts">
	import type { Snippet } from 'svelte';

	let { title, children }: { title: string; children?: Snippet } = $props();
</script>

<div class="space-y-2">
	<h1 class="text-3xl font-semibold tracking-tight">{title}</h1>
	{#if children}
		<div class="text-sm text-muted-foreground">{@render children()}</div>
	{/if}
</div>
