<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';
	import type { Snippet } from 'svelte';
	import { cn } from '$lib/utils.js';

	let {
		class: className,
		children,
		...restProps
	}: HTMLAttributes<HTMLElement> & { children?: Snippet } = $props();
</script>

<nav
	aria-label="breadcrumb"
	class={cn('basis-full w-full text-sm md:w-auto md:basis-auto', className)}
	{...restProps}
>
	{@render children?.()}
</nav>
