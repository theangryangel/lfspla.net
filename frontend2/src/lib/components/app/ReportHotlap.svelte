<script lang="ts">
	import FlagIcon from '@lucide/svelte/icons/flag';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import {
		useApi,
		apiErrorMessage,
		type HotlapReportSummary,
		type HotlapCommentResponse,
		type ReportReason,
	} from '#lib/api.js';
	import { useSession } from '#lib/session.svelte.js';
	import { reportReasons } from '#lib/report-reasons.js';
	import { goto, invalidate } from '$app/navigation';
	let {
		hotlap,
		existingReport = null,
		label,
		open = $bindable(false),
		hideTrigger = false,
	}: {
		hotlap: number;
		existingReport?: HotlapReportSummary | null;
		label?: string;
		open?: boolean;
		hideTrigger?: boolean;
	} = $props();
	const api = useApi();
	const session = useSession();
	let own = $state<HotlapCommentResponse | null>(null);
	let reason = $state<ReportReason | ''>('');
	let body = $state('');
	let checking = $state(false);
	let loaded = $state(false);
	let busy = $state(false);
	let error = $state('');
	$effect(() => {
		if (!open || !session.signedIn) return;
		let active = true;
		checking = true;
		loaded = false;
		own = null;
		error = '';
		api.reports
			.getMyReport({ hotlap })
			.then((report) => {
				if (active) {
					loaded = true;
					own = report ?? null;
					reason = report?.reason ?? '';
					body = report?.body ?? '';
				}
			})
			.catch((cause) => {
				if (active) error = apiErrorMessage(cause);
			})
			.finally(() => {
				if (active) checking = false;
			});
		return () => {
			active = false;
		};
	});
	async function save() {
		if (busy || checking || !loaded || !reason || !body.trim()) return;
		busy = true;
		error = '';
		try {
			const auth = { hotlap, 'X-CSRF-Token': session.me.csrf_token };
			const creating = !own;
			const result = await api.reports.saveMyReport({ ...auth, reason, body });
			open = false;
			await Promise.all([
				invalidate('app:reports'),
				invalidate('app:comments'),
			]);
			if (creating)
				await goto(
					`/hotlap/${hotlap}?tab=conversation&around=${result.id}#comment-${result.id}`,
				);
		} catch (cause) {
			error = apiErrorMessage(cause);
		} finally {
			busy = false;
		}
	}
</script>

<Dialog.Root bind:open>
	{#if !hideTrigger}
		<Dialog.Trigger>
			{#snippet child({ props })}
				<Button
					{...props}
					variant="ghost"
					size={label ? 'sm' : 'icon'}
					aria-label={label ?? 'Flag this lap or edit your flag'}
					title={existingReport?.total
						? `${existingReport.total} reports · flag this lap or edit your flag`
						: 'Flag this lap'}
				>
					<FlagIcon
						class={existingReport?.unresolved
							? 'size-4 text-primary'
							: 'size-4'}
						fill={existingReport?.unresolved ? 'currentColor' : 'none'}
					/>
					{#if label}{label}{/if}
				</Button>
			{/snippet}
		</Dialog.Trigger>
	{/if}
	<Dialog.Content class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-xl">
		<Dialog.Header>
			<Dialog.Title
				>{own ? 'Edit your report' : 'Report this hotlap'}</Dialog.Title
			>
			<Dialog.Description
				>Your report starts a public conversation on this hotlap. Other
				signed-in players can reply. You can have one report per lap.</Dialog.Description
			>
		</Dialog.Header>
		{#if !session.signedIn}
			<p class="text-sm text-muted-foreground">
				Sign in to flag hotlap #{hotlap}.
			</p>
			<Button onclick={session.signIn}>Sign in with LFS</Button>
		{:else if checking}
			<p role="status" class="text-sm text-muted-foreground">
				Loading your flag…
			</p>
		{:else if !loaded}
			<p class="text-sm text-muted-foreground">
				Your flag could not be loaded. Close this dialog and try again.
			</p>
		{:else}
			<form
				class="grid gap-4"
				onsubmit={(event) => {
					event.preventDefault();
					save();
				}}
			>
				{#if own?.resolved_at}<p class="text-sm text-muted-foreground">
						A moderator has resolved your flag. Changing your reason will send
						it for review again.
					</p>{/if}
				<label class="grid gap-2 text-sm font-medium"
					>Reason
					<select
						class="w-full rounded border bg-background px-3 py-2 font-normal"
						bind:value={reason}
						required
						disabled={busy}
					>
						<option value="" disabled>Choose a reason</option>
						{#each Object.entries(reportReasons) as [value, text] (value)}<option
								{value}>{text}</option
							>{/each}
					</select>
				</label>
				<label class="grid gap-2 text-sm font-medium"
					>Explain the problem
					<textarea
						class="min-h-32 w-full rounded border bg-background px-3 py-2 font-normal"
						bind:value={body}
						required
						maxlength={4000}
						disabled={busy}></textarea>
				</label>
				<Dialog.Footer class="flex-wrap">
					<Button
						type="button"
						variant="ghost"
						disabled={busy}
						onclick={() => (open = false)}>Cancel</Button
					>
					<Button type="submit" disabled={busy || !reason || !body.trim()}
						>{busy ? 'Saving…' : own ? 'Save changes' : 'Submit report'}</Button
					>
				</Dialog.Footer>
			</form>
		{/if}
		{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
	</Dialog.Content>
</Dialog.Root>
