<script lang="ts">
	import Empty from '#lib/components/app/Empty.svelte';
	import TrackLocationCard from '#lib/components/app/TrackLocationCard.svelte';
	import TrackCredits from '#lib/components/app/TrackCredits.svelte';
	import CornerRibbon from '#lib/components/app/CornerRibbon.svelte';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import PageHeading from '#lib/components/app/PageHeading.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<main id="main" class="w-full min-w-0 flex-1">
	<PageIdentity label="Track directory identity">
		{#snippet overlay()}<CornerRibbon />{/snippet}
		<div class="py-2">
			<PageHeading title="Tracks"
				>Explore LFS locations and their track configurations.</PageHeading
			>
		</div>
	</PageIdentity>
	<div class="mx-auto w-full max-w-screen-2xl space-y-6 p-4 md:p-6">
		<aside
			aria-label="More track resources"
			class="space-y-2 border-l-2 border-primary bg-muted/50 px-4 py-3 text-sm text-muted-foreground"
		>
			<p>
				For Live for Speed track mods, layouts, objects, buildings, textures and
				tools, check out <a
					class="text-primary underline underline-offset-4"
					href="https://lfstrackhub.com/"
					target="_blank"
					rel="noopener noreferrer">LFS Track Hub</a
				>.
			</p>
		</aside>
		{#if data.locations.length}
			<ul aria-label="Track locations" class="space-y-4">
				{#each data.locations as location (location.code)}
					<li><TrackLocationCard {location} /></li>
				{/each}
			</ul>
		{:else}
			<Empty title="No locations available" />
		{/if}
		<TrackCredits />
	</div>
</main>
