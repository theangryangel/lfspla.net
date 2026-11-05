<script lang="ts">
	import type { Snippet } from 'svelte';
	import { cn } from '#lib/utils.js';

	let { children, class: className }: { children: Snippet; class?: string } =
		$props();
</script>

<div
	class={cn(
		'record-highlights grid min-w-0 grid-cols-2 gap-2 lg:gap-x-0',
		className,
	)}
>
	{@render children()}
</div>

<style>
	@media (min-width: 64rem) {
		.record-highlights {
			/* Overlap the angled surfaces, including the first panel in each row. */
			padding-left: var(--highlight-slant);
		}
		.record-highlights :global(.record-highlight) {
			margin-left: calc(-1 * var(--highlight-slant));
		}
	}
</style>
