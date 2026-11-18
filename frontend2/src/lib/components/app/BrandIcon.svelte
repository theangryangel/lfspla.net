<script lang="ts">
	import { siTwitch, siYoutube, siDiscord } from 'simple-icons';
	import { cn } from '#lib/utils.js';

	let {
		name,
		class: className,
	}: { name: 'twitch' | 'youtube' | 'discord'; class?: string } = $props();
	const icon = $derived(
		name === 'twitch' ? siTwitch : name === 'youtube' ? siYoutube : siDiscord,
	);
</script>

<svg
	viewBox="0 0 24 24"
	fill="currentColor"
	class={cn('size-4 shrink-0', className)}
	aria-hidden="true"
>
	<path d={icon.path} />
</svg>
