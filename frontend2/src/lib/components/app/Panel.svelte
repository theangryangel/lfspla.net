<script lang="ts">
	import type { Snippet } from 'svelte';
	import * as Card from '$lib/components/ui/card/index.js';
	import { Button } from '$lib/components/ui/button/index.js';

	let {
		title,
		titleId,
		description,
		to,
		flush = false,
		surface = true,
		appearance = 'default',
		action,
		children,
	}: {
		title: string;
		titleId?: string;
		description?: string;
		to?: string;
		flush?: boolean;
		surface?: boolean;
		appearance?: 'default' | 'data';
		action?: Snippet;
		children: Snippet;
	} = $props();
</script>

<section class="space-y-3">
	<header class="flex flex-wrap items-start justify-between gap-3">
		<div class="space-y-1">
			<h2
				id={titleId}
				class={appearance === 'data'
					? 'racing-section-title flex h-8 items-center'
					: 'text-lg font-semibold'}
			>
				{title}
			</h2>
			{#if description}<p class="text-sm text-muted-foreground">
					{description}
				</p>{/if}
		</div>
		{#if to}<Button variant="link" href={to}>View all →</Button>{/if}
		{#if action}{@render action()}{/if}
	</header>
	{#if surface}
		<Card.Root class="gap-0 pb-0">
			<Card.Content class={flush ? 'p-0' : 'space-y-4 py-4'}
				>{@render children()}</Card.Content
			>
		</Card.Root>
	{:else}
		<div class={flush ? '' : 'space-y-4 py-4'}>{@render children()}</div>
	{/if}
</section>
