<script lang="ts">
	import { page } from '$app/state';
	import { LfsplanetApiError } from '@lfsplanet/sdk';
	import {
		apiErrorMessage,
		useApi,
		type Hotlap,
		type PaginatedResponseHotlap,
		type HotlapListColumn,
		type Ordering,
	} from '$lib/api.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import {
		dateTime,
		delta,
		hotlapStates,
		lapTime,
		steeringLabels,
	} from '$lib/format.js';

	import DownloadIcon from '@lucide/svelte/icons/download';
	import InfoIcon from '@lucide/svelte/icons/info';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import { invalidate } from '$app/navigation';

	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import TableFrame from '$lib/components/app/TableFrame.svelte';
	import * as Table from '$lib/components/ui/table/index.js';
	import Empty from '$lib/components/app/Empty.svelte';

	import { useSession } from '$lib/session.svelte.js';

	import PaginationControls from '$lib/components/app/PaginationControls.svelte';
	import SortableHead from '$lib/components/app/SortableHead.svelte';

	const api = useApi();

	let {
		hotlaps: submissions,
	}: {
		hotlaps: PaginatedResponseHotlap | null;
	} = $props();

	const session = useSession();
	const submittedDate = new Intl.DateTimeFormat(undefined, {
		day: 'numeric',
		month: 'short',
		year: 'numeric',
	});
	/** The submission being withdrawn, and why the last attempt did not work. */
	let removing = $state<number | null>(null);
	let removeError = $state('');
	let refreshError = $state('');
	let confirmOpen = $state(false);
	let removeTarget = $state<Hotlap | null>(null);
	let validating = $state<number | null>(null);
	let validateError = $state('');
	const busy = $derived(removing !== null || validating !== null);

	async function validate(hotlap: Hotlap) {
		if (busy) return;
		validating = hotlap.id;
		validateError = '';
		refreshError = '';
		try {
			await api.hotlaps.validateForTesting({
				'X-CSRF-Token': session.me.csrf_token,
				hotlap: hotlap.id,
			});
			try {
				await invalidate('app:hotlaps');
			} catch {
				refreshError =
					'Your change was saved, but the list could not refresh. Please reload.';
			}
		} catch (error) {
			validateError =
				error instanceof LfsplanetApiError
					? apiErrorMessage(error)
					: 'The submission could not be validated.';
		} finally {
			validating = null;
		}
	}

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

{#snippet details(hotlap: Hotlap)}
	<Dialog.Root>
		<Dialog.Trigger>
			{#snippet child({ props })}
				<Button
					{...props}
					variant="ghost"
					size="icon"
					aria-label="Details for {replayName(hotlap)}"><InfoIcon /></Button
				>
			{/snippet}
		</Dialog.Trigger>
		<Dialog.Content
			class="max-h-[calc(100dvh-4rem)] overflow-y-auto sm:max-w-lg"
		>
			<Dialog.Header class="pr-8">
				<Dialog.Title>Submission details</Dialog.Title>
				<Dialog.Description
					>{hotlap.era_title} / {hotlap.track} / {hotlap.vehicle ||
						hotlap.submission?.raw_vehicle_name ||
						'Unknown vehicle'}</Dialog.Description
				>
			</Dialog.Header>
			<dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-3 text-sm">
				<dt class="text-muted-foreground">Submitted</dt>
				<dd>{dateTime(hotlap.created_at)}</dd>
				<dt class="text-muted-foreground">Lap time</dt>
				<dd class="font-mono font-semibold tabular-nums">
					{lapTime(hotlap.lap_time_ms)}
				</dd>
				<dt class="text-muted-foreground">Rank</dt>
				<dd class="tabular-nums">{hotlap.position ?? '-'}</dd>
				<dt class="text-muted-foreground">To WR</dt>
				<dd class="font-mono tabular-nums">
					{delta(hotlap.distance_to_world_record_ms)}
				</dd>
				<dt class="text-muted-foreground">Controller</dt>
				<dd>{steeringLabels[hotlap.steering]}</dd>
				<dt class="text-muted-foreground">File</dt>
				<dd class="break-all">{hotlap.spr?.original_filename ?? '-'}</dd>
				<dt class="text-muted-foreground">Status</dt>
				<dd>
					<Badge variant={hotlapStates[hotlap.state].variant}
						>{hotlapStates[hotlap.state].label}</Badge
					>
					{#if hotlap.submission?.error_detail}<p
							class="mt-1 break-words text-muted-foreground"
						>
							{hotlap.submission.error_detail}
						</p>{/if}
				</dd>
				<dt class="text-muted-foreground">Contributes to</dt>
				<dd class="flex flex-wrap gap-1">
					{#each hotlap.contributes_to ?? [] as ranking (ranking.id)}
						<Badge
							variant="outline"
							href={`/hotlaps/${hotlap.era_id}/rankings/${encodeURIComponent(ranking.id)}`}
							>{ranking.title}</Badge
						>
					{:else}<span class="text-muted-foreground">-</span>{/each}
				</dd>
			</dl>
			{#if hotlap.raf || (session.me.allow_test_validation && hotlap.state !== 'valid')}
				<Dialog.Footer>
					{#if hotlap.raf}<Button variant="outline" href={hotlap.raf.url}
							><DownloadIcon />Download RAF</Button
						>{/if}
					{#if session.me.allow_test_validation && hotlap.state !== 'valid'}
						<Button
							variant="outline"
							disabled={busy || !hotlap.vehicle}
							onclick={() => validate(hotlap)}
							>{validating === hotlap.id
								? 'Validating...'
								: 'Force validate'}</Button
						>
					{/if}
				</Dialog.Footer>
			{/if}
		</Dialog.Content>
	</Dialog.Root>
{/snippet}

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
	{#if validateError}
		<p
			class="rounded-sm border border-destructive/40 px-4 py-3 text-sm text-destructive"
			role="alert"
		>
			{validateError}
		</p>
	{/if}
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
									<time
										datetime={hotlap.created_at}
										title={dateTime(hotlap.created_at)}
									>
										{submittedDate.format(new Date(hotlap.created_at))}
									</time>
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
									>{lapTime(hotlap.lap_time_ms)}</Table.Cell
								>
								<Table.Cell>
									<Badge variant={hotlapStates[hotlap.state].variant}
										>{hotlapStates[hotlap.state].label}</Badge
									>
									{#if hotlap.submission?.error_detail}
										<p
											class="max-w-xs whitespace-normal text-xs text-muted-foreground"
										>
											{hotlap.submission.error_detail}
										</p>
									{/if}
								</Table.Cell>
								<Table.Cell>
									<div class="flex items-center gap-1">
										{#if hotlap.spr?.url}
											<Button
												variant="ghost"
												size="icon"
												href={hotlap.spr.url}
												aria-label="Download {replayName(hotlap)}"
												><DownloadIcon /></Button
											>
										{/if}
										{@render details(hotlap)}
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

<Dialog.Root bind:open={confirmOpen}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>Are you sure?</Dialog.Title>
			<Dialog.Description>
				Remove {removeTarget ? replayName(removeTarget) : 'this submission'}?
				This cannot be undone.
			</Dialog.Description>
		</Dialog.Header>
		{#if removeError}
			<p role="alert" class="text-sm text-destructive">{removeError}</p>
		{/if}
		<Dialog.Footer>
			<Button
				variant="outline"
				disabled={busy}
				onclick={() => {
					confirmOpen = false;
				}}>Cancel</Button
			>
			<Button variant="destructive" disabled={busy} onclick={remove}>
				{removing !== null ? 'Removing...' : 'Remove submission'}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
