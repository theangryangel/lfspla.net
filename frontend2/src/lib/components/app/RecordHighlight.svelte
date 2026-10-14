<script lang="ts">
	import type { Snippet } from 'svelte';
	let {
		title,
		children,
		variant = 'yellow',
	}: {
		title: string;
		children: Snippet;
		variant?: 'yellow' | 'purple';
	} = $props();
</script>

<section
	class="record-highlight relative grid min-w-0 gap-2 px-14 py-3 lg:w-80 lg:py-4"
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
		clip-path: polygon(2.25rem 0, 100% 0, calc(100% - 2.25rem) 100%, 0 100%);
	}
	.record-highlight :global(a:focus-visible) {
		outline: 2px solid currentColor;
		outline-offset: 3px;
	}
</style>
