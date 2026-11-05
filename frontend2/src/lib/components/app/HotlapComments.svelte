<script lang="ts">
	import {
		useApi,
		apiErrorMessage,
		type PaginatedResponseHotlapCommentResponse,
	} from '#lib/api.js';
	import { useSession } from '#lib/session.svelte.js';
	import { invalidate, goto } from '$app/navigation';
	import { page } from '$app/state';
	import CommentThread from './CommentThread.svelte';
	import Empty from './Empty.svelte';
	import SendIcon from '@lucide/svelte/icons/send';
	import PaginationControls from './PaginationControls.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	let {
		hotlap,
		comments,
	}: {
		hotlap: number;
		comments: PaginatedResponseHotlapCommentResponse;
	} = $props();
	const api = useApi();
	const session = useSession();
	let body = $state('');
	let busy = $state(false);
	let error = $state('');
	let draftLap = $state<number | null>(null);
	$effect(() => {
		if (draftLap !== hotlap) {
			draftLap = hotlap;
			body = '';
			error = '';
		}
	});
	async function post() {
		if (busy || !body.trim()) return;
		const id = hotlap;

		busy = true;
		error = '';
		try {
			const result = await api.comments.createHotlapComment({
				hotlap: id,
				body,
				'X-CSRF-Token': session.me.csrf_token,
			});
			if (hotlap === id) {
				body = '';
				await Promise.all([
					invalidate('app:comments'),
					invalidate('app:reports'),
				]);
				const url = new URL(page.url.href);
				url.searchParams.delete('comment');
				url.searchParams.set('around', String(result.id));
				url.searchParams.delete('tab');
				url.searchParams.delete('comments_page');
				url.hash = `comment-${result.id}`;
				await goto(url);
			}
		} catch (cause) {
			error = apiErrorMessage(cause);
		} finally {
			busy = false;
		}
	}
</script>

<section
	id="comments"
	aria-label="Conversation"
	class="min-w-0 space-y-4 scroll-mt-6"
>
	<div class="space-y-4">
		{#each comments.items as comment (comment.id)}
			<CommentThread entry={comment} />
		{:else}
			<Empty title="No comments yet" />
		{/each}
	</div>
	{#if comments.pagination.total_items > 0}<PaginationControls
			pagination={comments.pagination}
			label="conversations"
			pageKey="comments_page"
		/>{/if}
	{#if session.signedIn}
		<form
			class="space-y-3"
			onsubmit={(event) => {
				event.preventDefault();
				post();
			}}
		>
			<label class="grid gap-2 text-sm font-medium"
				>Add a comment<textarea
					class="block min-h-24 w-full resize-y rounded-lg border border-input bg-transparent px-3 py-2 text-sm outline-none transition-colors placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 dark:bg-input/30"
					placeholder="Write a comment…"
					bind:value={body}
					maxlength={4000}
					required
					disabled={busy}></textarea></label
			>
			<div class="flex flex-wrap items-center justify-between gap-3">
				<p class="text-xs text-muted-foreground">
					Comments are public. Use plain text.
				</p>
				<Button type="submit" size="sm" disabled={busy || !body.trim()}
					><SendIcon class="size-4" />{busy
						? 'Posting…'
						: 'Post comment'}</Button
				>
			</div>
		</form>
	{:else}<Button variant="outline" onclick={session.signIn}
			>Sign in to comment</Button
		>{/if}
	{#if error}<p role="alert" class="text-sm text-destructive">
			{error}
		</p>{/if}
</section>
