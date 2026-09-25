<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';
	import type { Snippet } from 'svelte';
	import { cn } from '$lib/utils.js';

	let {
		class: className,
		children,
		...restProps
	}: HTMLAttributes<HTMLOListElement> & { children?: Snippet } = $props();
</script>

<ol
	class={cn(
		'flex flex-wrap items-center gap-1.5 break-words text-muted-foreground sm:gap-2.5',
		className,
	)}
	{...restProps}
>
	{@render children?.()}
</ol>
