<script lang="ts">
	import type { Snippet } from 'svelte';
	let {
		children,
		secondary,
		balanced = false,
		secondaryLabelledBy,
	}: {
		children: Snippet;
		secondary: Snippet;
		balanced?: boolean;
		secondaryLabelledBy: string;
	} = $props();
</script>

<div
	class="racing-data grid min-w-0 items-start gap-6"
	class:xl:grid-cols-2={balanced}
	class:xl:grid-cols-[minmax(0,1fr)_24rem]={!balanced}
>
	<div class="min-w-0">{@render children()}</div>
	<aside
		class="min-w-0 border-t bg-card/60 px-4 pb-4 pt-4 xl:border-t-0 xl:border-l xl:pt-0"
		aria-labelledby={secondaryLabelledBy}
	>
		{@render secondary()}
	</aside>
</div>
