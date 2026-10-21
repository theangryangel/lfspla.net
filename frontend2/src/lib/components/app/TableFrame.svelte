<script lang="ts">
	import type { Snippet } from 'svelte';
	import { cn } from '#lib/utils.js';

	let {
		class: className,
		appearance = 'default',
		children,
	}: {
		class?: string;
		appearance?: 'default' | 'data';
		children: Snippet;
	} = $props();
</script>

<div
	data-slot="table-frame"
	class={cn(
		'overflow-hidden text-card-foreground',
		appearance === 'data'
			? 'racing-data rounded-none border-y bg-background'
			: 'rounded-xl bg-card ring-1 ring-foreground/10',
		className,
	)}
>
	{@render children()}
</div>
