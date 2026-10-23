<script lang="ts">
	import DateTime from '#lib/components/app/DateTime.svelte';
	import type { EventSummary } from '#lib/api.js';
	import Thumbnail from './Thumbnail.svelte';
	import { CARD } from './picker.js';
	import { cn } from '#lib/utils.js';

	let { event }: { event: EventSummary } = $props();
	const title = $derived(
		[event.league.name, event.season?.name, event.title]
			.filter(Boolean)
			.join(' / '),
	);
</script>

{#snippet content()}
	<div class="flex w-full items-start">
		<Thumbnail
			src={event.league.image?.url}
			fit="contain"
			class="m-3 aspect-square w-20 shrink-0 rounded-md sm:w-32"
		/>
		<div
			class="flex min-w-0 flex-1 flex-col justify-center gap-2 p-3 pl-0 sm:p-4 sm:pl-1"
		>
			<h2
				class="text-base font-semibold break-words group-hover:text-primary group-focus-visible:text-primary sm:text-lg"
			>
				{title}
			</h2>
			<dl class="flex flex-wrap gap-x-4 gap-y-1 text-sm">
				<div>
					<dt class="inline text-muted-foreground">Start:</dt>
					<dd class="inline">
						<DateTime value={event.starts_at} />
					</dd>
				</div>
				{#if event.track_label}<div>
						<dt class="inline text-muted-foreground">Track:</dt>
						<dd class="inline">{event.track_label}</dd>
					</div>{/if}
				{#if event.cars_label}<div>
						<dt class="inline text-muted-foreground">Cars:</dt>
						<dd class="inline">{event.cars_label}</dd>
					</div>{/if}
			</dl>
			{#if event.description}<p
					class="whitespace-pre-line text-sm text-muted-foreground wrap-anywhere"
				>
					{event.description}
				</p>{/if}
		</div>
	</div>
{/snippet}

{#if event.details_url}
	<a
		href={event.details_url}
		target="_blank"
		rel="noopener noreferrer"
		aria-label={title}
		class={cn(
			CARD,
			'group flex bg-card text-card-foreground focus-visible:outline-offset-2',
		)}
	>
		{@render content()}
	</a>
{:else}
	<article
		class="flex w-full overflow-hidden rounded-lg border bg-card text-left text-card-foreground"
	>
		{@render content()}
	</article>
{/if}
