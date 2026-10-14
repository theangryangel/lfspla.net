<script lang="ts">
	import type { Snippet } from 'svelte';
	let {
		label,
		children,
		highlight,
		overlay,
	}: {
		label: string;
		children?: Snippet;
		highlight?: Snippet;
		overlay?: Snippet;
	} = $props();
</script>

<header
	class="page-identity relative border-b bg-card text-card-foreground"
	aria-label={label}
>
	{@render overlay?.()}
	<div
		class="mx-auto grid w-full max-w-screen-2xl items-stretch gap-x-8 px-4 md:px-6"
		class:lg:grid-cols-[minmax(0,1fr)_auto]={!!children && !!highlight}
	>
		{#if children}<div
				class="flex min-w-0 flex-col justify-center py-3 lg:py-4"
			>
				{@render children()}
			</div>{/if}
		{#if highlight}<div class="identity-highlights grid min-w-0 pb-4 lg:pb-0">
				{@render highlight()}
			</div>{/if}
	</div>
</header>

<style>
	.page-identity {
		container-type: inline-size;
		--identity-gutter: 1rem;
	}
	.identity-highlights {
		--identity-bleed: calc(
			max(0px, (100cqw - 96rem) / 2) + var(--identity-gutter)
		);
	}
	.identity-highlights :global(.record-highlight) {
		align-content: center;
	}
	@media (min-width: 48rem) {
		.page-identity {
			--identity-gutter: 1.5rem;
		}
	}
	@media (min-width: 64rem) {
		/* Let the final desktop surface reach the header edge. */
		.identity-highlights :global(.record-highlight:last-child)::before {
			right: calc(-1 * var(--identity-bleed));
			clip-path: polygon(var(--highlight-slant) 0, 100% 0, 100% 100%, 0 100%);
		}
	}
</style>
