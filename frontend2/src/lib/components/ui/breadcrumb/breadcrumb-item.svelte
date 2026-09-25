<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';
	import type { Snippet } from 'svelte';
	import { cn } from '$lib/utils.js';

	let {
		class: className,
		children,
		...restProps
	}: HTMLAttributes<HTMLLIElement> & { children?: Snippet } = $props();
</script>

<li class={cn('inline-flex items-center gap-1.5', className)} {...restProps}>
	{@render children?.()}
</li>
