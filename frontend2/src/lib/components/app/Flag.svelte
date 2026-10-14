<script lang="ts">
	import { cn } from '$lib/utils.js';

	let {
		code,
		fallback,
		size = 'sm',
		class: className,
	}: {
		code?: string | null;
		fallback?: string | null;
		size?: 'sm' | 'lg';
		class?: string;
	} = $props();

	const resolvedCode = $derived(code || fallback);
	const slug = $derived(resolvedCode?.toLowerCase() ?? '');
	const width = $derived(size === 'lg' ? 40 : 20);
	const height = $derived(size === 'lg' ? 30 : 15);
</script>

{#if resolvedCode}
	<img
		src="https://flagcdn.com/{width}x{height}/{slug}.png"
		srcset="https://flagcdn.com/{width * 2}x{height * 2}/{slug}.png 2x"
		{width}
		{height}
		loading="lazy"
		alt="{resolvedCode} flag"
		title={resolvedCode}
		class={cn(
			'inline-block shrink-0 rounded-[1px] align-middle shadow-sm',
			className,
		)}
	/>
{/if}
