<script lang="ts">
	import DateTime from '#lib/components/app/DateTime.svelte';
	import { page } from '$app/state';
	import { LfsplanetApiError } from '@lfsplanet/sdk';
	import {
		apiErrorMessage,
		useApi,
		type Hotlap,
		type PaginatedResponseHotlap,
		type HotlapListColumn,
		type Ordering,
	} from '#lib/api.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { hotlapStates, lapTime } from '#lib/format.js';

	import HotlapLinks from '#lib/components/app/HotlapLinks.svelte';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import { invalidate } from '$app/navigation';

	import ConfirmDialog from '#lib/components/app/ConfirmDialog.svelte';
	import TableFrame from '#lib/components/app/TableFrame.svelte';
	import * as Table from '#lib/components/ui/table/index.js';
	import Empty from '#lib/components/app/Empty.svelte';

	import { useSession } from '#lib/session.svelte.js';

	import PaginationControls from '#lib/components/app/PaginationControls.svelte';
	import SortableHead from '#lib/components/app/SortableHead.svelte';

	const api = useApi();

	let {
		hotlaps: submissions,
	}: {
		hotlaps: PaginatedResponseHotlap | null;
	} = $props();

	const session = useSession();
	/** The submission being withdrawn, and why the last attempt did not work. */
	let removing = $state<number | null>(null);
	let removeError = $state('');
	let refreshError = $state('');
	let confirmOpen = $state(false);
	let removeTarget = $state<Hotlap | null>(null);
	const busy = $derived(removing !== null);

	const replayName = (hotlap: Hotlap) =>
		hotlap.spr?.original_filename ??
		`${hotlap.track} / ${hotlap.vehicle ?? hotlap.submission?.raw_vehicle_name}`;

	/**
	 * Withdraws one submission, in whatever state it reached.
	 *
	 * Uploads are capped per player while they are being processed, so removing
	 * a rejected replay is also how a driver makes room for the next one.
	 */
	async function remove() {
		if (!removeTarget || busy) return;
		const hotlap = removeTarget;
		removing = hotlap.id;
		removeError = '';
		refreshError = '';
		try {
			await api.hotlaps.remove({
				'X-CSRF-Token': session.me.csrf_token,
				hotlap: hotlap.id,
			});
			confirmOpen = false;
			removeTarget = null;
			try {
				await invalidate('app:hotlaps');
			} catch {
				refreshError =
					'Your change was saved, but the list could not refresh. Please reload.';
			}
		} catch (error) {
			removeError =
				error instanceof LfsplanetApiError
					? apiErrorMessage(error)
					: 'The submission could not be removed.';
		} finally {
			removing = null;
		}
	}

	const hotlaps = $derived(submissions?.items ?? []);
	const column = $derived(
		((page.url.searchParams.get('column') ?? '') ||
			'submitted') as HotlapListColumn,
	);
	const order = $derived(
		((page.url.searchParams.get('order') ?? '') || 'desc') as Ordering,
	);
</script>

{#if refreshError}<p role="alert" class="text-destructive">
		{refreshError}
	</p>{/if}
{#if !session.signedIn}
	<Empty title="Sign in to see your submissions">
		<p>Your uploaded replays and their validation results appear here.</p>
		<Button onclick={session.signIn}>Sign in with LFS</Button>
	</Empty>
{:else if submissions === null}
	<Empty title="Submissions are unavailable"
		><p>Please try again shortly.</p></Empty
	>
{:else}
	{#if hotlaps.length}
		<TableFrame appearance="data">
			<div
				class="[&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4 [&_caption]:px-4 [&_caption]:pb-4"
			>
				<Table.Root aria-label="My hotlap submissions">
					<Table.Header>
						<Table.Row>
							<SortableHead
								column="submitted"
								label="Submitted"
								activeColumn={column}
								{order}
							/>
							<Table.Head>Era / Track / Vehicle</Table.Head>
							<Table.Head class="text-center">Rank</Table.Head>
							<SortableHead
								column="lap_time"
								label="Lap time"
								activeColumn={column}
								{order}
							/>
							<Table.Head>Status</Table.Head>
							<Table.Head><span class="sr-only">Actions</span></Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each hotlaps as hotlap (hotlap.id)}
							<Table.Row>
								<Table.Cell class="text-muted-foreground">
									<DateTime value={hotlap.created_at} format="date" />
								</Table.Cell>
								<Table.Cell>
									{#if hotlap.vehicle}
										<a
											class="hover:underline"
											href={`/hotlaps/${hotlap.era_id}/charts/${encodeURIComponent(hotlap.track)}/${encodeURIComponent(hotlap.vehicle)}`}
										>
											{hotlap.era_title} / {hotlap.track} / {hotlap.vehicle}
										</a>
									{:else}
										{hotlap.era_title} / {hotlap.track} / {hotlap.submission
											?.raw_vehicle_name ?? 'Unknown vehicle'}
									{/if}
								</Table.Cell>
								<Table.Cell class="text-center tabular-nums"
									>{hotlap.position ?? '-'}</Table.Cell
								>
								<Table.Cell class="font-mono font-semibold tabular-nums"
									><a class="hover:underline" href={`/hotlap/${hotlap.id}`}
										>{lapTime(hotlap.lap_time_ms)}</a
									></Table.Cell
								>
								<Table.Cell>
									<Badge variant={hotlapStates[hotlap.state].variant}
										>{hotlapStates[hotlap.state].label}</Badge
									>
									{#if hotlap.submission?.error_detail}
										<p
											class="max-w-xs whitespace-normal text-sm text-muted-foreground"
										>
											{hotlap.submission.error_detail}
										</p>
									{/if}
								</Table.Cell>
								<Table.Cell>
									<div class="flex items-center gap-1">
										<HotlapLinks {hotlap} />
										<Button
											variant="ghost"
											size="icon"
											disabled={busy}
											aria-label="Remove {replayName(hotlap)}"
											onclick={() => {
												removeTarget = hotlap;
												removeError = '';
												confirmOpen = true;
											}}><Trash2Icon class="text-destructive" /></Button
										>
									</div>
								</Table.Cell>
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
			</div>
		</TableFrame>
	{:else}
		<Empty
			title={submissions.pagination.total_items
				? 'No submissions on this page'
				: 'No submissions yet'}
		>
			<p>
				{submissions.pagination.total_items
					? 'Choose an earlier page below.'
					: 'Uploaded replays will appear here with their validation results.'}
			</p>
		</Empty>
	{/if}
	<PaginationControls pagination={submissions.pagination} label="submissions" />
{/if}

<ConfirmDialog
	bind:open={confirmOpen}
	title="Remove submission?"
	description={`Remove ${removeTarget ? replayName(removeTarget) : 'this submission'}? This cannot be undone.`}
	confirmLabel="Remove submission"
	{busy}
	error={removeError}
	onConfirm={remove}
/>
