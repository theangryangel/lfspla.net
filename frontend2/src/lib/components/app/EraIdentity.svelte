<script lang="ts">
	import type { EraSummary } from '$lib/api.js';
	import PageIdentity from './PageIdentity.svelte';
	let { era }: { era: EraSummary } = $props();
</script>

<PageIdentity label="Era identity">
	<div class="space-y-3 py-2">
		<h1 class="racing-page-title">{era.title}</h1>
		<p class="max-w-prose text-sm text-muted-foreground">
			{#if era.open}
				This era is open for hotlap submissions and accepts replays recorded
				with LFS {era.version_requirement}.
			{:else}
				This historical era accepts replays recorded with LFS {era.version_requirement}.
			{/if}
		</p>
	</div>
</PageIdentity>
