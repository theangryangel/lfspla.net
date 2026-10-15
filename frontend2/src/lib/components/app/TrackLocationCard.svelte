<script lang="ts">
	import type { TrackLocation } from '$lib/api.js';
	import Thumbnail from './Thumbnail.svelte';

	let { location }: { location: TrackLocation } = $props();
</script>

<a
	href={`/tracks/${encodeURIComponent(location.code)}`}
	class="group flex overflow-hidden rounded-lg border bg-card text-card-foreground transition-colors hover:border-primary/50 hover:bg-accent/30 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
>
	<div class="flex w-full flex-col sm:flex-row">
		<Thumbnail
			src={location.image?.url}
			class="w-full shrink-0 sm:w-64 lg:w-80"
		/>
		<div class="flex min-w-0 flex-1 flex-col justify-center gap-3 p-4 sm:p-6">
			<div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
				<h2 class="text-xl font-semibold group-hover:text-primary">
					{location.name}
				</h2>
				<span class="font-mono text-sm text-muted-foreground"
					>{location.code}</span
				>
			</div>
			{#if location.description}<p
					class="max-w-prose whitespace-pre-line text-sm text-muted-foreground"
				>
					{location.description}
				</p>{/if}
		</div>
	</div>
</a>
