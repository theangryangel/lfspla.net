<script lang="ts">
	import CameraOffIcon from '@lucide/svelte/icons/camera-off';
	import { cn } from '$lib/utils.js';

	let {
		src = null,
		class: className,
	}: {
		src?: string | null;
		class?: string;
	} = $props();
</script>

<div
	class={cn(
		'relative flex aspect-16/10 items-center justify-center overflow-hidden bg-muted',
		className,
	)}
>
	{#if !src}
		<CameraOffIcon
			class="h-1/3 w-auto text-muted-foreground/40"
			aria-hidden="true"
		/>
	{:else}
		<img {src} alt="" loading="lazy" class="size-full object-contain p-[5%]" />
	{/if}
</div>
