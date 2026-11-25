<script lang="ts">
	import {
		apiErrorMessage,
		useApi,
		type TeamInviteResponse,
	} from '#lib/api.js';
	import { useSession } from '#lib/session.svelte.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import ChoiceSelect from './ChoiceSelect.svelte';
	import DateTime from './DateTime.svelte';

	let { teamId, open = $bindable(false) }: { teamId: number; open?: boolean } =
		$props();
	const api = useApi();
	const session = useSession();
	let invites = $state<TeamInviteResponse[]>([]);
	let loadedTeam = $state<number | null>(null);
	let loading = $state(false);
	let pending = $state(false);
	let error = $state('');
	let days = $state('7');
	let uses = $state('');
	let link = $state('');
	let createdId = $state<number | null>(null);
	let message = $state('');
	let now = $state(Date.now());
	let generation = 0;

	async function load() {
		const current = ++generation;
		loading = true;
		error = '';
		now = Date.now();
		try {
			const result = await api.teams.listTeamInvites({ id: teamId });
			if (current === generation) {
				invites = result.items;
				loadedTeam = teamId;
			}
		} catch (cause) {
			if (current === generation) error = apiErrorMessage(cause);
		} finally {
			if (current === generation) loading = false;
		}
	}
	$effect(() => {
		if (open) {
			teamId;
			void load();
		} else {
			generation++;
			loadedTeam = null;
			link = '';
			createdId = null;
			message = '';
		}
	});
	function active(invite: TeamInviteResponse) {
		return (
			!invite.revoked_at &&
			(!invite.expires_at || Date.parse(invite.expires_at) > now) &&
			(invite.max_uses == null || invite.uses < invite.max_uses)
		);
	}
	async function create(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		pending = true;
		error = '';
		message = '';
		try {
			const result = await api.teams.createTeamInvite({
				id: teamId,
				'X-CSRF-Token': session.me.csrf_token,
				expires_in_days: days ? Number(days) : undefined,
				max_uses: uses ? Number(uses) : undefined,
			});
			link = `${window.location.origin}/teams/join/${result.token}`;
			createdId = result.invite.id;
			invites = [result.invite, ...invites];
		} catch (cause) {
			error = apiErrorMessage(cause);
		} finally {
			pending = false;
		}
	}
	async function copy() {
		try {
			await navigator.clipboard.writeText(link);
			message = 'Invite link copied.';
		} catch {
			message = 'Select the link and copy it manually.';
		}
	}
	async function revoke(invite: TeamInviteResponse) {
		if (pending) return;
		pending = true;
		error = '';
		message = '';
		try {
			await api.teams.revokeTeamInvite({
				id: teamId,
				invite_id: invite.id,
				'X-CSRF-Token': session.me.csrf_token,
			});
			invites = invites.map((item) =>
				item.id === invite.id
					? { ...item, revoked_at: new Date().toISOString() }
					: item,
			);
			if (createdId === invite.id) {
				link = '';
				createdId = null;
			}
			message = 'Invite link revoked.';
		} catch (cause) {
			error = apiErrorMessage(cause);
		} finally {
			pending = false;
		}
	}
</script>

<Dialog.Root
	{open}
	onOpenChange={(value) => {
		if (!pending) open = value;
	}}
>
	<Dialog.Content
		class="max-h-[85dvh] overflow-y-auto sm:max-w-lg"
		showCloseButton={!pending}
	>
		<Dialog.Header
			><Dialog.Title>Invite members</Dialog.Title><Dialog.Description
				>Share a private link. Players sign in and confirm before joining.</Dialog.Description
			></Dialog.Header
		>
		{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
		{#if link}
			<div class="space-y-2 rounded-lg border p-3">
				<label for="team-invite-link" class="text-sm font-medium"
					>Your invite link</label
				>
				<div class="flex items-center gap-2">
					<Input
						id="team-invite-link"
						value={link}
						readonly
						onclick={(event) => event.currentTarget.select()}
					/><Button onclick={copy}>Copy link</Button>
				</div>
				<p class="text-sm text-muted-foreground">
					Copy it before closing. The full link is only shown once.
				</p>
				<Button
					variant="ghost"
					size="sm"
					onclick={() => {
						link = '';
						createdId = null;
					}}>Create another link</Button
				>
			</div>
		{:else}
			<form onsubmit={create} class="space-y-4">
				<fieldset
					disabled={pending || loading || loadedTeam !== teamId}
					class="space-y-4"
				>
					<div class="grid gap-4 sm:grid-cols-2">
						<div class="space-y-2">
							<span class="text-sm font-medium">Expires after</span
							><ChoiceSelect
								label="Invite expiry"
								value={days}
								options={[
									{ value: '1', label: '1 day' },
									{ value: '7', label: '7 days' },
									{ value: '30', label: '30 days' },
									{ value: '', label: 'Never' },
								]}
								onValueChange={(value) => (days = value)}
							/>
						</div>
						<div class="space-y-2">
							<label for="team-invite-uses" class="text-sm font-medium"
								>Maximum joins</label
							><Input
								id="team-invite-uses"
								type="number"
								min={1}
								max={10000}
								placeholder="Unlimited"
								bind:value={uses}
							/>
						</div>
					</div>
					<Button
						type="submit"
						disabled={pending || loading || loadedTeam !== teamId}
						>{pending ? 'Creating...' : 'Create invite link'}</Button
					>
				</fieldset>
			</form>
		{/if}
		{#if message}<p role="status" class="text-sm text-muted-foreground">
				{message}
			</p>{/if}
		<section class="space-y-3 border-t pt-4" aria-labelledby="team-invite-list">
			<h3 id="team-invite-list" class="text-sm font-semibold">Invite links</h3>
			{#if loading}<p role="status" class="text-sm text-muted-foreground">
					Loading invite links...
				</p>
			{:else if loadedTeam !== teamId}<Button variant="outline" onclick={load}
					>Retry</Button
				>
			{:else if invites.length}<ul class="max-h-64 divide-y overflow-y-auto">
					{#each invites as invite (invite.id)}
						<li class="flex items-center justify-between gap-3 py-3">
							<div class="min-w-0 space-y-1 text-sm">
								<div class="flex flex-wrap items-center gap-2">
									<span>Created <DateTime value={invite.created_at} /></span
									><Badge variant="outline"
										>{invite.revoked_at
											? 'Revoked'
											: active(invite)
												? 'Active'
												: 'Expired or used'}</Badge
									>
								</div>
								<p class="text-muted-foreground">
									{invite.uses}
									{invite.max_uses == null
										? 'joins · Unlimited uses'
										: `of ${invite.max_uses} joins`}
								</p>
								{#if invite.expires_at}<p class="text-muted-foreground">
										Expires <DateTime value={invite.expires_at} />
									</p>{/if}
							</div>
							{#if active(invite)}<Button
									variant="outline"
									size="sm"
									disabled={pending}
									onclick={() => revoke(invite)}>Revoke</Button
								>{/if}
						</li>
					{/each}
				</ul>{:else}<p class="text-sm text-muted-foreground">
					No invite links yet.
				</p>{/if}
		</section>
	</Dialog.Content>
</Dialog.Root>
