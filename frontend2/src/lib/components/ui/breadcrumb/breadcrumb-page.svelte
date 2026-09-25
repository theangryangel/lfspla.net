<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';
	import type { Snippet } from 'svelte';
	import { cn } from '$lib/utils.js';

	let {
		class: className,
		children,
		...restProps
	}: HTMLAttributes<HTMLSpanElement> & { children?: Snippet } = $props();
</script>

<span
	role="link"
	aria-current="page"
	aria-disabled="true"
	class={cn('font-normal text-foreground', className)}
	{...restProps}
>
	{@render children?.()}
</span>
