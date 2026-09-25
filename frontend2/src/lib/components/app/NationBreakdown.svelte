<script lang="ts">
	import Info from '@lucide/svelte/icons/info';
	import { mergeProps } from 'bits-ui';
	import { getList, type NationContribution } from '$lib/api.js';
	import { delta } from '$lib/format.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import Flag from './Flag.svelte';

	let {
		era,
		ranking,
		country,
	}: { era: string; ranking: string; country: string } = $props();
	let open = $state(false);
	let pending = $state(false);
	let failed = $state(false);
	let retry = $state(0);
	let rows = $state<NationContribution[]>([]);
	const points = $derived(rows.reduce((sum, row) => sum + row.points, 0));
	const handicap = $derived(
		rows.reduce((sum, row) => sum + row.handicap_ms, 0),
	);

	$effect(() => {
		if (!open) return;
		const path = `/api/v1/eras/${encodeURIComponent(era)}/rankings/${encodeURIComponent(ranking)}/nations/${encodeURIComponent(country)}/contributors`;
		retry;
		let active = true;
		pending = true;
		failed = false;
		rows = [];
		getList<NationContribution>(fetch, path)
			.then((result) => {
				if (active) rows = result;
			})
			.catch(() => {
				if (active) failed = true;
			})
			.finally(() => {
				if (active) pending = false;
			});
		return () => {
			active = false;
		};
	});
</script>

<Dialog.Root bind:open>
	<Tooltip.Root>
		<Tooltip.Trigger>
			<!--
				The tooltip and the dialog both drive this one button, so their props
				are merged rather than spread in sequence: `mergeProps` chains the
				event handlers each needs instead of letting the last one win.
			-->
			{#snippet child({ props: tooltipProps })}
				<Dialog.Trigger>
					{#snippet child({ props: dialogProps })}
						<Button
							{...mergeProps(tooltipProps, dialogProps)}
							variant="ghost"
							size="icon-sm"
							aria-label={`Show ${country} score breakdown`}
						>
							<Info />
						</Button>
					{/snippet}
				</Dialog.Trigger>
			{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Content>Score breakdown</Tooltip.Content>
	</Tooltip.Root>
	<Dialog.Content class="sm:max-w-2xl">
		<Dialog.Header class="pr-8">
			<Dialog.Title
				><span class="inline-flex items-center gap-2"
					><Flag code={country} /> {country} score breakdown</span
				></Dialog.Title
			>
			<Dialog.Description
				>Each driver's contribution from laps that count towards this nation's
				score. Handicap is the contribution to the nation's tie-break.</Dialog.Description
			>
		</Dialog.Header>
		{#if pending}
			<p role="status" class="py-6 text-center text-muted-foreground">
				Loading contributions...
			</p>
		{:else if failed}
			<div class="space-y-3">
				<p role="alert" class="text-destructive">
					Could not load the score breakdown.
				</p>
				<Button variant="outline" onclick={() => retry++}>Try again</Button>
			</div>
		{:else if rows.length}
			<p class="text-sm text-muted-foreground">
				{points.toLocaleString()} points · {delta(handicap)} handicap
			</p>
			<div class="max-h-[60vh] overflow-auto rounded-lg border">
				<Table.Root aria-label={`${country} player contributions`}>
					<Table.Header>
						<Table.Row>
							<Table.Head>Driver</Table.Head>
							<Table.Head class="text-right">Points</Table.Head>
							<Table.Head class="text-right">Scoring charts</Table.Head>
							<Table.Head class="text-right">Handicap</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each rows as row (row.player_id)}
							<Table.Row>
								<Table.Cell
									><a
										class="font-medium hover:underline"
										href={`/drivers/${encodeURIComponent(row.lfs_username)}`}
										onclick={() => (open = false)}>{row.display_name}</a
									></Table.Cell
								>
								<Table.Cell class="text-right tabular-nums"
									>{row.points.toLocaleString()}</Table.Cell
								>
								<Table.Cell class="text-right tabular-nums"
									>{row.contributing_charts.toLocaleString()}</Table.Cell
								>
								<Table.Cell class="text-right font-mono tabular-nums"
									>{delta(row.handicap_ms)}</Table.Cell
								>
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
			</div>
		{:else}
			<p class="py-6 text-center text-muted-foreground">
				No scoring contributions for this nation.
			</p>
		{/if}
	</Dialog.Content>
</Dialog.Root>
