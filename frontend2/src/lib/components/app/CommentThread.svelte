<script lang="ts">
	import { tick } from 'svelte';
	import { invalidate } from '$app/navigation';
	import {
		useApi,
		apiErrorMessage,
		type HotlapCommentResponse,
	} from '#lib/api.js';
	import { useSession } from '#lib/session.svelte.js';
	import ConversationEntry from './ConversationEntry.svelte';
	import EntryActions from './EntryActions.svelte';
	import ConfirmDialog from './ConfirmDialog.svelte';
	import ReportHotlap from './ReportHotlap.svelte';
	import DateTime from './DateTime.svelte';
	import CommentThread from './CommentThread.svelte';
	import { reportReasons } from '#lib/report-reasons.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import FlagIcon from '@lucide/svelte/icons/flag';
	import ReplyIcon from '@lucide/svelte/icons/reply';
	let {
		entry,
		indent = 0,
	}: {
		entry: HotlapCommentResponse;
		indent?: number;
	} = $props();
	const api = useApi();
	const session = useSession();
	const own = $derived(
		session.signedIn && session.player?.id === entry.author.id,
	);
	let busy = $state(false);
	let error = $state('');
	let confirmation = $state<'delete' | 'withdraw'>('delete');
	let confirmOpen = $state(false);
	let editOpen = $state(false);
	let replyOpen = $state(false);
	let body = $state('');
	let expanded = $state(true);
	let loading = $state(false);
	let extraReplies = $state<HotlapCommentResponse[]>([]);
	let knownTotal = $state<number | null>(null);
	const children = $derived(
		Array.from(
			new Map(
				[...entry.replies, ...extraReplies].map((row) => [row.id, row]),
			).values(),
		).sort((a, b) => a.id - b.id),
	);
	const totalReplies = $derived(knownTotal ?? entry.reply_count);
	$effect(() => {
		entry;
		extraReplies = [];
		knownTotal = null;
	});
	async function loadMore() {
		if (loading) return;
		loading = true;
		error = '';
		try {
			const response = await api.comments.listHotlapComments({
				parent_id: entry.id,
				hotlap: entry.original_hotlap_id,
				page: Math.floor(children.length / 50) + 1,
			});
			extraReplies = [...extraReplies, ...response.items];
			knownTotal = response.pagination.total_items;
		} catch (cause) {
			error = apiErrorMessage(cause);
		} finally {
			loading = false;
		}
	}
	async function refresh() {
		await Promise.all([invalidate('app:comments'), invalidate('app:reports')]);
	}
	async function remove() {
		if (busy) return;
		busy = true;
		error = '';
		try {
			const auth = { 'X-CSRF-Token': session.me.csrf_token };
			if (confirmation === 'withdraw')
				await api.comments.updateHotlapComment({
					reason: null,
					hotlap: entry.original_hotlap_id,
					comment: entry.id,
					...auth,
				});
			else
				await api.comments.removeHotlapComment({
					hotlap: entry.original_hotlap_id,
					comment: entry.id,
					...auth,
				});
			confirmOpen = false;
			await refresh();
		} catch (cause) {
			error = apiErrorMessage(cause);
		} finally {
			busy = false;
		}
	}
	async function resolve() {
		if (busy) return;
		busy = true;
		error = '';
		try {
			await api.comments.updateHotlapComment({
				hotlap: entry.original_hotlap_id,
				comment: entry.id,
				resolved: !entry.resolved_at,
				'X-CSRF-Token': session.me.csrf_token,
			});
			await refresh();
		} catch (cause) {
			error = apiErrorMessage(cause);
		} finally {
			busy = false;
		}
	}
	async function reply() {
		if (busy || !body.trim() || entry.hotlap_id === null) return;
		busy = true;
		error = '';
		try {
			const result = await api.comments.createHotlapComment({
				hotlap: entry.hotlap_id,
				parent_id: entry.id,
				body,
				'X-CSRF-Token': session.me.csrf_token,
			});
			body = '';
			replyOpen = false;
			expanded = true;
			await refresh();
			await tick();
			extraReplies = [...extraReplies, result];
			await tick();
			document
				.getElementById(`comment-${result.id}`)
				?.scrollIntoView({ block: 'nearest' });
		} catch (cause) {
			error = apiErrorMessage(cause);
		} finally {
			busy = false;
		}
	}
</script>

<div class="min-w-0 space-y-3">
	<ConversationEntry
		author={entry.author}
		createdAt={entry.created_at}
		id={`comment-${entry.id}`}
		report={Boolean(entry.reason)}
		resolved={Boolean(entry.resolved_at)}
	>
		{#snippet actions()}
			{#if !entry.deleted && (own || (session.isModerator && entry.reason))}
				<EntryActions label="Comment actions" disabled={busy}>
					{#if own}
						{#if entry.reason}
							{#if entry.hotlap_id !== null}<DropdownMenu.Item
									onSelect={() => (editOpen = true)}
									>Edit report</DropdownMenu.Item
								>{/if}
							<DropdownMenu.Item
								onSelect={() => {
									error = '';
									confirmation = 'withdraw';
									confirmOpen = true;
								}}>Withdraw report</DropdownMenu.Item
							>
						{/if}
						<DropdownMenu.Item
							variant="destructive"
							onSelect={() => {
								error = '';
								confirmation = 'delete';
								confirmOpen = true;
							}}>Delete comment</DropdownMenu.Item
						>
					{/if}
					{#if session.isModerator && entry.reason}
						{#if own}<DropdownMenu.Separator />{/if}
						<DropdownMenu.Item onSelect={resolve}
							>{entry.resolved_at
								? 'Reopen report'
								: 'Resolve report'}</DropdownMenu.Item
						>
					{/if}
				</EntryActions>
			{/if}
		{/snippet}
		{#if entry.reason}
			<div class="flex flex-wrap items-center gap-2 text-sm">
				<FlagIcon
					class={entry.resolved_at
						? 'size-5 text-destructive/50'
						: 'size-5 text-destructive'}
					fill="currentColor"
				/><span class="font-semibold">{reportReasons[entry.reason]}</span>
				<Badge
					variant="outline"
					class={entry.resolved_at
						? 'border-destructive/20 text-muted-foreground'
						: 'border-destructive/50 text-destructive'}
					>{entry.resolved_at ? 'Resolved report' : 'Awaiting review'}</Badge
				>
			</div>
		{/if}
		{#if entry.deleted}<p class="text-sm italic text-muted-foreground">
				Comment deleted.
			</p>{:else}<p
				class="whitespace-pre-wrap break-words text-sm leading-relaxed"
			>
				{entry.body}
			</p>{/if}
		{#if entry.updated_at !== entry.created_at && !entry.deleted}<p
				class="text-xs text-muted-foreground"
			>
				Edited <DateTime value={entry.updated_at} />
			</p>{/if}
		{#if entry.reason && entry.resolved_at}<p
				class="text-xs text-muted-foreground"
			>
				Resolved by a moderator <DateTime value={entry.resolved_at} />
			</p>{/if}
		<div class="flex flex-wrap items-center gap-2">
			{#if session.signedIn && entry.hotlap_id !== null && entry.depth < 8}<Button
					variant="ghost"
					size="sm"
					disabled={busy}
					onclick={() => (replyOpen = !replyOpen)}
					><ReplyIcon class="size-4" />Reply</Button
				>{/if}
			{#if totalReplies}<Button
					variant="ghost"
					size="sm"
					onclick={() => (expanded = !expanded)}
					>{expanded
						? 'Hide replies'
						: `Show ${totalReplies} ${totalReplies === 1 ? 'reply' : 'replies'}`}</Button
				>{/if}
		</div>
		{#if replyOpen}
			<form
				class="space-y-3 pt-3"
				onsubmit={(event) => {
					event.preventDefault();
					reply();
				}}
			>
				<label class="grid gap-2 text-sm font-medium"
					>Reply to {entry.author.display_name ||
						entry.author.lfs_username}<textarea
						class="block min-h-24 w-full resize-y rounded-lg border border-input bg-transparent px-3 py-2 text-sm outline-none transition-colors placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 dark:bg-input/30"
						bind:value={body}
						required
						maxlength={4000}
						disabled={busy}></textarea></label
				>
				<div class="flex justify-end gap-2">
					<Button
						type="button"
						size="sm"
						variant="ghost"
						disabled={busy}
						onclick={() => (replyOpen = false)}>Cancel</Button
					><Button type="submit" size="sm" disabled={busy || !body.trim()}
						>{busy ? 'Posting…' : 'Post reply'}</Button
					>
				</div>
			</form>
		{/if}
		{#if error && !confirmOpen}<p role="alert" class="text-sm text-destructive">
				{error}
			</p>{/if}
	</ConversationEntry>
	{#if expanded}
		<div
			class={indent < 3
				? 'ml-3 min-w-0 space-y-4 sm:ml-6'
				: 'min-w-0 space-y-4'}
			aria-label="Replies"
		>
			{#each children as child (child.id)}<CommentThread
					entry={child}
					indent={indent + 1}
				/>{/each}
			{#if children.length < totalReplies}
				<Button
					variant="outline"
					size="sm"
					disabled={loading}
					onclick={loadMore}
					>{loading ? 'Loading replies…' : 'Load more replies'}</Button
				>
			{/if}
		</div>
	{/if}
</div>
<ConfirmDialog
	bind:open={confirmOpen}
	title={confirmation === 'withdraw'
		? 'Withdraw your report?'
		: 'Delete your comment?'}
	description={confirmation === 'withdraw'
		? 'This removes the report flag. Your comment and its replies remain in the conversation.'
		: 'Your text will be removed and replaced with a deleted-comment placeholder. Replies will remain. This cannot be undone.'}
	confirmLabel={confirmation === 'withdraw'
		? 'Withdraw report'
		: 'Delete comment'}
	pendingLabel="Saving…"
	{busy}
	{error}
	onConfirm={remove}
/>
{#if entry.hotlap_id !== null && entry.reason}<ReportHotlap
		hotlap={entry.hotlap_id}
		hideTrigger
		bind:open={editOpen}
	/>{/if}
