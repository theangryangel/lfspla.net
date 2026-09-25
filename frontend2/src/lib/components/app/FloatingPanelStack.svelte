<script lang="ts">
	import { useFloatingPanels } from '$lib/floating-panels.svelte.js';
	const panels = useFloatingPanels();
	const ordered = $derived(
		[...panels.entries()].sort((a, b) => a[1].order - b[1].order),
	);
	const bottom = $derived(
		ordered.filter(([, panel]) => panel.placement === 'bottom'),
	);
	const topRight = $derived(
		ordered.filter(([, panel]) => panel.placement === 'top-right'),
	);
	let height = $state(0);
</script>

{#if bottom.length}
	<div
		style:height={`${height + 32}px`}
		class="shrink-0"
		aria-hidden="true"
	></div>
	<div
		bind:clientHeight={height}
		class="fixed right-4 bottom-4 left-4 z-30 mx-auto flex max-h-[70dvh] max-w-4xl flex-col gap-3 overflow-y-auto overscroll-contain rounded-lg"
	>
		{#each bottom as [id, panel] (id)}
			<div class="shrink-0">{@render panel.content()}</div>
		{/each}
	</div>
{/if}

{#if topRight.length}
	<div
		class="fixed top-4 right-4 z-30 flex w-[calc(100%-2rem)] max-w-md flex-col gap-3 overflow-y-auto overscroll-contain rounded-lg"
		style:max-height={`calc(100dvh - ${bottom.length ? height + 48 : 32}px)`}
	>
		{#each topRight as [id, panel] (id)}
			<div class="shrink-0">{@render panel.content()}</div>
		{/each}
	</div>
{/if}
