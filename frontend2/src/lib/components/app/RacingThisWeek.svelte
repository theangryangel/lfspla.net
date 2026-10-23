<script lang="ts">
	import ArrowRight from '@lucide/svelte/icons/arrow-right';
	import DateTime from '#lib/components/app/DateTime.svelte';
	import type { EventSummary } from '#lib/api.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import Panel from './Panel.svelte';
	import Thumbnail from './Thumbnail.svelte';
	import Empty from './Empty.svelte';

	let { events }: { events: EventSummary[] | null } = $props();
	const days = $derived.by(() => {
		const groups = new Map<string, EventSummary[]>();
		for (const event of events ?? []) {
			const day = new Date(event.starts_at).toDateString();
			const group = groups.get(day);
			if (group) group.push(event);
			else groups.set(day, [event]);
		}
		return Array.from(groups, ([key, items]) => ({ key, items }));
	});
</script>

{#if events === null || events.length}
	<Panel title="Racing this week" appearance="data" surface={false} flush>
		{#if events === null}
			<Empty title="Events couldn’t be loaded">
				<p>Please try again later.</p>
			</Empty>
		{:else}
			<ul class="racing-data text-sm">
				{#each days as day (day.key)}
					<li>
						<h3 class="border-b bg-muted/50 px-3 py-2 font-medium">
							<DateTime value={day.items[0].starts_at} format="date" />
						</h3>
						<ul class="divide-y">
							{#each day.items as event (event.id)}
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
											<DateTime
												value={event.starts_at}
												format="time"
												class="text-muted-foreground tabular-nums"
											/>
											<h4
												class="font-medium wrap-anywhere group-hover:text-primary"
											>
												{event.league.name}
											</h4>
											<p class="text-muted-foreground wrap-anywhere">
												{[event.season?.name, event.title]
													.filter(Boolean)
													.join(' / ')}
											</p>
										</div>
									</a>
								</li>
							{/each}
						</ul>
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
