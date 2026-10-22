<script lang="ts">
	import ArrowRight from '@lucide/svelte/icons/arrow-right';
	import DateTime from '#lib/components/app/DateTime.svelte';
	import type { EventSummary } from '#lib/api.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import Panel from './Panel.svelte';
	import Thumbnail from './Thumbnail.svelte';
	import Empty from './Empty.svelte';

	let { events }: { events: EventSummary[] | null } = $props();
</script>

{#if events === null || events.length}
	<Panel title="Racing this week" appearance="data" surface={false} flush>
		{#if events === null}
			<Empty title="Events couldn’t be loaded">
				<p>Please try again later.</p>
			</Empty>
		{:else}
			<ul class="divide-y border-y">
				{#each events as event (event.id)}
					<li>
						<a
							href={event.details_url ?? '/events'}
							class="group flex min-w-0 items-center gap-3 px-3 py-3 hover:bg-muted/50 focus-visible:outline-2 focus-visible:outline-ring focus-visible:outline-offset-[-2px]"
						>
							<Thumbnail
								src={event.league.image?.url}
								fit="contain"
								class="size-10 shrink-0 rounded-sm bg-transparent"
							/>
							<div class="flex min-w-0 flex-1 flex-col gap-1">
								<h3
									class="min-w-0 text-sm font-medium wrap-anywhere group-hover:text-primary"
								>
									{[event.league.name, event.season?.name, event.title]
										.filter(Boolean)
										.join(' / ')}
								</h3>
								<DateTime
									value={event.starts_at}
									class="shrink-0 text-sm text-muted-foreground tabular-nums"
								/>
							</div>
						</a>
					</li>
				{/each}
			</ul>
		{/if}
		<div class="flex justify-end pt-2">
			<Button variant="link" href="/events">
				All events <ArrowRight class="size-4" aria-hidden="true" />
			</Button>
		</div>
	</Panel>
{/if}
