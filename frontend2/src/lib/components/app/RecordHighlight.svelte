<script lang="ts">
	import type { Snippet } from 'svelte';
	import { cn } from '#lib/utils.js';
	let {
		title,
		showTitle = true,
		children,
		variant = 'yellow',
		compact = false,
		flushEnd = false,
		flushStart = false,
		class: className,
	}: {
		title: string;
		showTitle?: boolean;
		children: Snippet;
		variant?: 'yellow' | 'purple' | 'cyan' | 'neutral';
		compact?: boolean;
		flushEnd?: boolean;
		flushStart?: boolean;
		class?: string;
	} = $props();
</script>

<section
	class={cn(
		'record-highlight relative grid min-w-0',
		compact
			? 'gap-1 px-10 py-2 lg:w-56 lg:py-2'
			: 'gap-2 px-14 py-3 lg:w-80 lg:py-4',
		className,
	)}
	data-variant={variant}
	data-compact={compact || undefined}
	data-flush-end={flushEnd || undefined}
	data-flush-start={flushStart || undefined}
	aria-label={title}
>
	{#if showTitle}<h2 class="racing-section-title relative">{title}</h2>{/if}
	<div class="relative grid min-w-0 gap-2">{@render children()}</div>
</section>

<style>
	.record-highlight {
		--highlight-background: var(--brand-yellow);
		--highlight-pattern-tint: 90%;
		color: black;
	}
	.record-highlight[data-compact] > h2 {
		font-size: 1.25rem;
	}
	.record-highlight[data-variant='purple'] {
		--highlight-background: oklch(0.541 0.247 293);
		color: white;
	}
	.record-highlight[data-variant='cyan'] {
		--highlight-background: var(--primary);
		color: var(--primary-foreground);
	}
	.record-highlight[data-variant='neutral'] {
		--highlight-background: var(--border);
		--highlight-pattern-tint: 95%;
		color: var(--foreground);
	}
	.record-highlight::before {
		content: '';
		position: absolute;
		inset: 0;
		/* Fade a gently waving chequer into the panel's own colour.
		   All layers share this surface so overlaps and header bleed stay seamless. */
		background-color: var(--highlight-background);
		background-image:
			linear-gradient(
				115deg,
				rgb(255 255 255 / 0.06),
				transparent 42%,
				rgb(0 0 0 / 0.08)
			),
			linear-gradient(
				100deg,
				var(--highlight-background) 10%,
				color-mix(
					in srgb,
					var(--highlight-background) var(--highlight-pattern-tint),
					transparent
				)
			),
			url('/images/chequered-flag.svg');
		background-size: cover, cover, 16rem 8rem;
		background-position: center;
		pointer-events: none;
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
	.record-highlight[data-flush-end]::before {
		clip-path: polygon(var(--highlight-slant) 0, 100% 0, 100% 100%, 0 100%);
	}
	.record-highlight[data-flush-start]::before {
		clip-path: polygon(
			0 0,
			100% 0,
			calc(100% - var(--highlight-slant)) 100%,
			0 100%
		);
	}
	@media (width < 64rem) {
		.record-highlight {
			padding-inline: 1rem;
		}
		.record-highlight::before,
		.record-highlight[data-flush-end]::before,
		.record-highlight[data-flush-start]::before {
			clip-path: none;
		}
	}
</style>
