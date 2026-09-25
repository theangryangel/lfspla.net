<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';
	import type { Snippet } from 'svelte';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import { cn } from '$lib/utils.js';

	let {
		class: className,
		children,
		...restProps
	}: HTMLAttributes<HTMLLIElement> & { children?: Snippet } = $props();
</script>

<li
	role="presentation"
	aria-hidden="true"
	class={cn('text-muted-foreground', className)}
	{...restProps}
>
	{#if children}{@render children()}{:else}<ChevronRightIcon
			class="size-4"
		/>{/if}
</li>
