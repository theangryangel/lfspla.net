<script lang="ts">
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Card from '$lib/components/ui/card/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';
	import Progress from '$lib/components/app/Progress.svelte';
	import { useSession } from '$lib/session.svelte.js';
	import Empty from '$lib/components/app/Empty.svelte';
	import { queryValue, updateQuery } from '$lib/query.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const base = $derived(`/hotlaps/${data.era.id}/rankings`);
	const search = $derived(queryValue('q'));
	type Participation = 'all' | 'participating' | 'missed';

	const session = useSession();
	let participation = $state<Participation>('all');

	const visible = $derived(
		data.era.rankings
			.filter((ranking) => {
				const progress = data.progress?.[ranking.id];
				return (
					participation === 'all' ||
					(participation === 'participating' &&
						(progress?.completed ?? 0) > 0) ||
					(participation === 'missed' &&
						progress !== undefined &&
						progress.completed < progress.total)
				);
			})
			.filter((ranking) =>
				`${ranking.title} ${ranking.description}`
					.toLowerCase()
					.includes(search.toLowerCase()),
			),
	);
</script>

<div class="flex flex-wrap items-center gap-4">
	<Input
		class="sm:max-w-xs"
		aria-label="Search rankings"
		placeholder="Search rankings..."
		value={search}
		oninput={(e) => updateQuery('q', e.currentTarget.value)}
	/>
	<ToggleGroup.Root
		type="single"
		variant="outline"
		aria-label="Ranking participation"
		bind:value={participation}
	>
		<ToggleGroup.Item value="all">All ranks</ToggleGroup.Item>
		<ToggleGroup.Item value="participating" disabled={!session.signedIn}
			>Participating</ToggleGroup.Item
		>
		<ToggleGroup.Item value="missed" disabled={!session.signedIn}
			>Missed</ToggleGroup.Item
		>
	</ToggleGroup.Root>
	<span class="text-sm text-muted-foreground sm:ml-auto">
		{visible.length} of {data.era.rankings.length} rankings
	</span>
</div>
{#if visible.length}
	<Card.Root class="gap-0 py-0">
		<Card.Content
			class="px-0 [&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4 [&_caption]:px-4 [&_caption]:pb-4"
		>
			<Table.Root>
				<Table.Header>
					<Table.Row>
						<Table.Head>Ranking</Table.Head>
						<Table.Head>Rank Info</Table.Head>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each visible as ranking (ranking.id)}
						<Table.Row>
							<Table.Cell>
								<a
									class="font-medium hover:underline"
									href="{base}/{ranking.id}"
								>
									{ranking.title}
								</a>
								<p class="text-muted-foreground">{ranking.description}</p>
							</Table.Cell>
							<Table.Cell>
								{@const progress = data.progress?.[ranking.id]}
								{#if !session.signedIn}
									<Button variant="ghost" size="sm" onclick={session.signIn}
										>Sign in to see progress</Button
									>
								{:else if progress}
									<a
										class="block rounded-sm focus-visible:outline-2 focus-visible:outline-ring"
										href="{base}/{ranking.id}/combinations"
										aria-label="{ranking.title}: your progress"
									>
										<Progress
											value={progress.completed}
											total={progress.total}
										/>
									</a>
								{:else}
									<a
										class="text-muted-foreground hover:underline"
										href="{base}/{ranking.id}/combinations"
										>Progress unavailable</a
									>
								{/if}
							</Table.Cell>
						</Table.Row>
					{/each}
				</Table.Body>
			</Table.Root>
		</Card.Content>
	</Card.Root>
{:else if data.era.rankings.length}
	<Empty title="No matching rankings">
		<p>Try another search.</p>
	</Empty>
{:else}
	<Empty title="No rankings in this era">
		<p>{data.era.title} does not offer any rankings yet.</p>
	</Empty>
{/if}
