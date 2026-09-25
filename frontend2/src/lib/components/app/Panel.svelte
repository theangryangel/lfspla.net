<script lang="ts">
	import type { Snippet } from 'svelte';
	import * as Card from '$lib/components/ui/card/index.js';
	import { Button } from '$lib/components/ui/button/index.js';

	let {
		title,
		description,
		to,
		flush = false,
		action,
		children,
	}: {
		title: string;
		description?: string;
		to?: string;
		flush?: boolean;
		action?: Snippet;
		children: Snippet;
	} = $props();
</script>

<Card.Root class="gap-0 pb-0">
	<Card.Header>
		<Card.Title>
			<h2>{title}</h2>
		</Card.Title>
		{#if description}<Card.Description>{description}</Card.Description>{/if}
		{#if to}
			<Card.Action>
				<Button variant="link" href={to}>View all →</Button>
			</Card.Action>
		{/if}
		{#if action}{@render action()}{/if}
	</Card.Header>
	<Card.Content class={flush ? 'p-0' : 'space-y-4 py-4'}
		>{@render children()}</Card.Content
	>
</Card.Root>
