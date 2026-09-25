<script lang="ts" module>
	export type { ArtworkKind as ThumbnailKind } from '$lib/assets/artwork.js';
</script>

<script lang="ts">
	import CameraOffIcon from '@lucide/svelte/icons/camera-off';
	import { artworkFor, type ArtworkKind } from '$lib/assets/artwork.js';
	import { cn } from '$lib/utils.js';

	let {
		kind,
		code,
		src = null,
		class: className,
	}: {
		kind: ArtworkKind;
		code: string;
		src?: string | null;
		class?: string;
	} = $props();

	const artwork = $derived(src ? { url: src } : artworkFor(kind, code));
</script>

<div
	class={cn(
		'relative flex aspect-16/10 items-center justify-center overflow-hidden bg-muted',
		className,
	)}
>
	{#if !artwork}
		<CameraOffIcon
			class="h-1/3 w-auto text-muted-foreground/40"
			aria-hidden="true"
		/>
	{:else if 'markup' in artwork}
		<div class="size-full [&>svg]:size-full" aria-hidden="true">
			{@html artwork.markup}
		</div>
	{:else}
		<img
			src={artwork.url}
			alt=""
			loading="lazy"
			class="size-full object-contain p-[5%]"
		/>
	{/if}
</div>
