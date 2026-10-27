<script lang="ts">
	import BrandIcon from '#lib/components/app/BrandIcon.svelte';
	import { page } from '$app/state';
	import { Button } from '#lib/components/ui/button/index.js';
	import type { ProfileLinks } from '#lib/api.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';

	let {
		links,
		selected,
		onClose,
	}: {
		links: ProfileLinks;
		selected: 'twitch' | 'youtube' | null;
		onClose: () => void;
	} = $props();
	let width = $state(0);
	let returnFocus: HTMLElement | null = null;
	let lastSelected = $state<'twitch' | 'youtube'>('twitch');
	$effect(() => {
		if (selected) lastSelected = selected;
	});
	const kind = $derived(selected ?? lastSelected);
	const twitch = $derived(`https://www.twitch.tv/${links.twitch_channel}`);
	const youtube = $derived(
		`https://www.youtube.com/watch?v=${links.youtube_video_id}`,
	);
	const source = $derived(
		kind === 'twitch'
			? `https://player.twitch.tv/?channel=${encodeURIComponent(links.twitch_channel ?? '')}&parent=${encodeURIComponent(page.url.hostname)}&autoplay=false`
			: `https://www.youtube.com/embed/${encodeURIComponent(links.youtube_video_id ?? '')}?autoplay=0`,
	);
</script>

<svelte:window bind:innerWidth={width} />

<Dialog.Root
	open={selected !== null}
	onOpenChange={(open) => {
		if (!open) onClose();
	}}
>
	<Dialog.Content
		id="driver-media"
		class="max-h-[calc(100dvh-2rem)] gap-0 overflow-y-auto p-0 sm:max-w-4xl"
		onOpenAutoFocus={() => {
			returnFocus =
				document.activeElement instanceof HTMLElement
					? document.activeElement
					: null;
		}}
		onCloseAutoFocus={(event) => {
			event.preventDefault();
			if (returnFocus?.isConnected) returnFocus.focus({ preventScroll: true });
		}}
	>
		<Dialog.Header
			class="flex-row items-center justify-between gap-2 px-4 py-2 pr-12"
		>
			<Dialog.Title class="flex min-w-0 items-center gap-2">
				<BrandIcon name={kind} />
				{kind === 'twitch' ? 'Twitch stream' : 'Featured video'}
			</Dialog.Title>
			<Button
				href={kind === 'twitch' ? twitch : youtube}
				target="_blank"
				rel="ugc nofollow noopener noreferrer"
				variant="ghost"
				size="sm"
				class="shrink-0"
				>Open in {kind === 'twitch' ? 'Twitch' : 'YouTube'}</Button
			>
		</Dialog.Header>
		{#if selected}
			<div class="w-full border-t">
				{#if kind === 'twitch' && width < 432}
					<div class="space-y-3 p-6 text-sm">
						<p class="text-muted-foreground">
							Watch this stream on Twitch for a better fit on your screen.
						</p>
					</div>
				{:else}
					{#key source}
						<iframe
							class="aspect-video min-h-[200px] w-full border-0 bg-black"
							class:min-h-[300px]={kind === 'twitch'}
							src={source}
							title={kind === 'twitch'
								? 'Driver Twitch stream'
								: 'Driver featured YouTube video'}
							allow="encrypted-media; fullscreen; picture-in-picture"
							allowfullscreen
						></iframe>
					{/key}
				{/if}
			</div>
		{/if}
	</Dialog.Content>
</Dialog.Root>
