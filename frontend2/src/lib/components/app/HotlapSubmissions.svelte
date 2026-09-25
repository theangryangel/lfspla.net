<script lang="ts">
	import DownloadIcon from '@lucide/svelte/icons/download';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import { invalidate } from '$app/navigation';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import * as Card from '$lib/components/ui/card/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import Empty from '$lib/components/app/Empty.svelte';
	import {
		dateTime,
		delta,
		hotlapStates,
		lapTime,
		steeringLabels,
	} from '$lib/format.js';
	import { queryValue } from '$lib/query.js';
	import { useSession } from '$lib/session.svelte.js';
	import { RequestFailed, send, type ManagedHotlapResponse } from '$lib/api.js';
	import type { EraSummary } from '$lib/api.js';
	import PaginationControls from '$lib/components/app/PaginationControls.svelte';
	import SortableHead from '$lib/components/app/SortableHead.svelte';
	import type {
		PaginatedResponse,
		HotlapListColumn,
		Ordering,
	} from '$lib/api.js';
	import { hotlapPath } from '$lib/era.js';

	let {
		hotlaps: submissions,
		era,
	}: {
		hotlaps: PaginatedResponse<ManagedHotlapResponse> | null;
		era?: EraSummary;
	} = $props();

	const session = useSession();
	/** The submission being withdrawn, and why the last attempt did not work. */
	let removing = $state<number | null>(null);
	let removeError = $state('');
	let refreshError = $state('');
	let confirmOpen = $state(false);
	let removeTarget = $state<ManagedHotlapResponse | null>(null);
	let validating = $state<number | null>(null);
	let validateError = $state('');
	const busy = $derived(removing !== null || validating !== null);

	async function validate(hotlap: ManagedHotlapResponse) {
		if (busy) return;
		validating = hotlap.id;
		validateError = '';
		refreshError = '';
		try {
			await send(`/api/v1/hotlaps/${hotlap.id}/validate`, {
				method: 'POST',
				csrf: session.me.csrf_token,
			});
			try {
				await invalidate('app:hotlaps');
			} catch {
				refreshError =
					'Your change was saved, but the list could not refresh. Please reload.';
			}
		} catch (error) {
			validateError =
				error instanceof RequestFailed
					? error.message
					: 'The submission could not be validated.';
		} finally {
			validating = null;
		}
	}

	const replayName = (hotlap: ManagedHotlapResponse) =>
		hotlap.original_filename ??
		`${hotlap.track} / ${hotlap.vehicle ?? hotlap.raw_vehicle_name}`;

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
			await send(`/api/v1/hotlaps/${hotlap.id}`, {
				method: 'DELETE',
				csrf: session.me.csrf_token,
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
				error instanceof RequestFailed
					? error.message
					: 'The submission could not be removed.';
		} finally {
			removing = null;
		}
	}

	const hotlaps = $derived(submissions?.items ?? []);
	const column = $derived(
		(queryValue('column') || 'submitted') as HotlapListColumn,
	);
	const order = $derived((queryValue('order') || 'desc') as Ordering);
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
	{#if validateError}
		<p
			class="rounded-lg border border-destructive/40 px-4 py-3 text-sm text-destructive"
			role="alert"
		>
			{validateError}
		</p>
	{/if}
	{#if hotlaps.length}
		<Card.Root class="gap-0 py-0">
			<Card.Content
				class="px-0 [&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4 [&_caption]:px-4 [&_caption]:pb-4"
			>
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<SortableHead
								column="submitted"
								label="Submitted"
								activeColumn={column}
								{order}
							/>
							{#if !era}<Table.Head>Era</Table.Head>{/if}
							<Table.Head>Track / Vehicle</Table.Head>
							<Table.Head class="text-center">#</Table.Head>
							<Table.Head>To WR</Table.Head>
							<Table.Head>Contributes to</Table.Head>
							<SortableHead
								column="lap_time"
								label="Lap time"
								activeColumn={column}
								{order}
							/>
							<Table.Head>Controller</Table.Head>
							<Table.Head>Status</Table.Head>
							<Table.Head>File</Table.Head>
							<Table.Head><span class="sr-only">Actions</span></Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each hotlaps as hotlap (hotlap.id)}
							<Table.Row>
								<Table.Cell class="text-muted-foreground"
									><time datetime={hotlap.created_at}
										>{dateTime(hotlap.created_at)}</time
									></Table.Cell
								>
								{#if !era}<Table.Cell
										><a class="hover:underline" href={hotlapPath(hotlap.era_id)}
											>{hotlap.era_title}</a
										></Table.Cell
									>{/if}
								<Table.Cell>
									{#if hotlap.vehicle}
										<a
											class="hover:underline"
											href={`${hotlapPath(hotlap.era_id)}/charts/${encodeURIComponent(hotlap.track)}/${encodeURIComponent(hotlap.vehicle)}`}
										>
											{hotlap.track} / {hotlap.vehicle}
										</a>
									{:else}
										{hotlap.track} / {hotlap.raw_vehicle_name}
									{/if}
								</Table.Cell>
								<Table.Cell class="text-center tabular-nums"
									>{hotlap.position ?? '-'}</Table.Cell
								>
								<Table.Cell class="font-mono tabular-nums"
									>{delta(hotlap.distance_to_world_record_ms)}</Table.Cell
								>
								<Table.Cell>
									<div class="flex flex-wrap gap-1">
										{#each hotlap.contributes_to ?? [] as ranking (ranking.id)}
											<Badge variant="outline">{ranking.title}</Badge>
										{:else}
											<span class="text-muted-foreground">-</span>
										{/each}
									</div>
								</Table.Cell>
								<Table.Cell class="font-mono tabular-nums">
									{lapTime(hotlap.lap_time_ms)}
								</Table.Cell>
								<Table.Cell class="text-muted-foreground">
									{steeringLabels[hotlap.steering]}
								</Table.Cell>
								<Table.Cell>
									<Badge variant={hotlapStates[hotlap.state].variant}>
										{hotlapStates[hotlap.state].label}
									</Badge>
									{#if hotlap.error_detail}
										<p class="text-xs text-muted-foreground">
											{hotlap.error_detail}
										</p>
									{/if}
								</Table.Cell>
								<Table.Cell class="text-muted-foreground">
									{hotlap.original_filename ?? '-'}
								</Table.Cell>
								<Table.Cell>
									{#if session.me.allow_test_validation && hotlap.state !== 'valid'}
										<Button
											variant="outline"
											size="sm"
											disabled={busy || !hotlap.vehicle}
											onclick={() => validate(hotlap)}
										>
											{validating === hotlap.id
												? 'Validating...'
												: 'Force validate'}
										</Button>
									{/if}
									{#if hotlap.replay_url}
										<Button
											variant="ghost"
											size="icon"
											href={hotlap.replay_url}
											aria-label="Download {replayName(hotlap)}"
										>
											<DownloadIcon />
										</Button>
									{/if}
									<Button
										variant="ghost"
										size="icon"
										disabled={busy}
										aria-label="Remove {replayName(hotlap)}"
										onclick={() => {
											removeTarget = hotlap;
											removeError = '';
											confirmOpen = true;
										}}
									>
										<Trash2Icon class="text-destructive" />
									</Button>
								</Table.Cell>
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
			</Card.Content>
		</Card.Root>
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
