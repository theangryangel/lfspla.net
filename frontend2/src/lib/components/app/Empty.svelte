<script lang="ts">
	import type { Snippet } from 'svelte';

	let { title, children }: { title: string; children?: Snippet } = $props();
</script>

<div class="flex min-w-0 flex-col items-center gap-3 border-y px-4 py-8 text-center">
	<h2 class="racing-section-title">{title}</h2>
	{#if children}
		<div class="max-w-prose space-y-3 text-sm text-muted-foreground">
			{@render children()}
		</div>
	{/if}
</div>
