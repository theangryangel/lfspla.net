<script lang="ts">
	import type { HTMLAnchorAttributes } from 'svelte/elements';
	import type { Snippet } from 'svelte';
	import { cn } from '$lib/utils.js';

	let {
		class: className,
		children,
		...restProps
	}: HTMLAnchorAttributes & { children?: Snippet } = $props();
</script>

<a
	class={cn(
		'transition-colors hover:text-foreground hover:underline',
		className,
	)}
	{...restProps}
>
	{@render children?.()}
</a>
