<script lang="ts">
	import { untrack } from 'svelte';
	import { createTeamsApi, teamErrorMessage } from '#lib/teams.js';
	import { useSession } from '#lib/session.svelte.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	let {
		team,
		initiallyEnabled,
		embedded = false,
		onEnabledChange,
	}: {
		team: string;
		initiallyEnabled: boolean;
		embedded?: boolean;
		onEnabledChange?: (enabled: boolean) => void;
	} = $props();
	const session = useSession();
	const api = createTeamsApi();
	const formId = $props.id();
	let enabled = $state(untrack(() => initiallyEnabled));
	let pending = $state(false);
	let inviteUrl = $state('');
	let error = $state('');
	let notice = $state('');

	async function update(disable = false) {
		if (pending || !session.signedIn) return;
		pending = true;
		error = '';
		notice = '';
		// A failed regeneration may still have reached the server; do not retain an old URL.
		inviteUrl = '';
		try {
			if (disable) {
				await api.disable(team, session.me.csrf_token);
				enabled = false;
				notice = 'Invite link disabled.';
			} else {
				const result = await api.regenerate(team, session.me.csrf_token);
				enabled = true;
				inviteUrl = new URL(
					`/teams/join/${encodeURIComponent(result.token)}`,
					window.location.origin,
				).href;
				notice = 'New invite link created. Previous links no longer work.';
			}
			onEnabledChange?.(enabled);
		} catch (cause) {
			error = teamErrorMessage(cause);
		} finally {
			pending = false;
		}
	}
	async function copy() {
		try {
			await navigator.clipboard.writeText(inviteUrl);
			notice = 'Invite link copied.';
		} catch {
			notice = 'Copying failed. Select the URL and copy it manually.';
		}
	}
</script>

<section
	class={embedded ? 'space-y-4' : 'space-y-4 rounded-lg border p-4'}
	aria-labelledby={`${formId}-title`}
>
	<h3 id={`${formId}-title`} class="text-lg font-semibold">
		Shared invite link
	</h3>
	<p class="text-sm text-muted-foreground">
		Anyone with this link can sign in and join as a member. Regenerating or
		disabling it invalidates the previous link.
	</p>
	<p class="text-sm">Invites are {enabled ? 'enabled' : 'disabled'}.</p>
	{#if enabled && !inviteUrl}<p class="text-sm text-muted-foreground">
			The existing URL cannot be retrieved. Regenerate it to get a new link.
		</p>{/if}
	<div class="flex flex-wrap gap-2">
		<Button disabled={pending || session.pending} onclick={() => update()}
			>{pending
				? 'Working...'
				: enabled
					? 'Regenerate link'
					: 'Create invite link'}</Button
		>
		<Button
			variant="outline"
			disabled={pending || session.pending || !enabled}
			onclick={() => update(true)}>Disable link</Button
		>
	</div>
	{#if inviteUrl}
		<div class="space-y-2">
			<label class="text-sm font-medium" for={`${formId}-url`}>Invite URL</label
			>
			<div class="flex flex-wrap gap-2">
				<Input
					id={`${formId}-url`}
					class="min-w-0 flex-1"
					value={inviteUrl}
					readonly
					onclick={(event) => event.currentTarget.select()}
				/>
				<Button variant="outline" onclick={copy}>Copy link</Button>
			</div>
			<p class="text-sm text-muted-foreground">
				Copy this URL now. It is only shown when created and disappears when
				these tools close.
			</p>
		</div>
	{/if}
	{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
	<p role="status" class="text-sm">{notice}</p>
</section>
