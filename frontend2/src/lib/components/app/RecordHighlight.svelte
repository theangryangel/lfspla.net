<script lang="ts">
	import type { Snippet } from 'svelte';
	import { cn } from '$lib/utils.js';
	let {
		title,
		children,
		variant = 'yellow',
		class: className,
	}: {
		title: string;
		children: Snippet;
		variant?: 'yellow' | 'purple';
		class?: string;
	} = $props();
</script>

<section
	class={cn(
		'record-highlight relative grid min-w-0 gap-2 px-14 py-3 lg:w-80 lg:py-4',
		className,
	)}
	data-variant={variant}
	aria-label={title}
>
	<h2 class="racing-section-title relative">{title}</h2>
	<div class="relative grid min-w-0 gap-2">{@render children()}</div>
</section>

<style>
	.record-highlight {
		--highlight-background: var(--brand-yellow);
		color: black;
	}
	.record-highlight[data-variant='purple'] {
		--highlight-background: oklch(0.541 0.247 293);
		color: white;
	}
	.record-highlight::before {
		content: '';
		position: absolute;
		inset: 0;
		background: var(--highlight-background);
		/* One direction for every highlight: bottom to the left, top to the right. */
		clip-path: polygon(
			var(--highlight-slant) 0,
			100% 0,
			calc(100% - var(--highlight-slant)) 100%,
			0 100%
		);
	}
	.record-highlight :global(a:focus-visible) {
		outline: 2px solid currentColor;
		outline-offset: 3px;
	}
</style>
