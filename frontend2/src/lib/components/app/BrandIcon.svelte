<script lang="ts">
	import { siTwitch, siYoutube } from 'simple-icons';
	import { cn } from '#lib/utils.js';

	let {
		name,
		class: className,
	}: { name: 'twitch' | 'youtube'; class?: string } = $props();
	const icon = $derived(name === 'twitch' ? siTwitch : siYoutube);
</script>

<svg
	viewBox="0 0 24 24"
	fill="currentColor"
	class={cn('size-4 shrink-0', className)}
	aria-hidden="true"
>
	<path d={icon.path} />
</svg>
