<script lang="ts">
	import { untrack } from 'svelte';
	import { invalidateAll } from '$app/navigation';
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import {
		createTeamsApi,
		teamErrorMessage,
		teamWebsite,
		type TeamDirectoryItem,
		type Team,
	} from '#lib/teams.js';
	import { useSession } from '#lib/session.svelte.js';
	import TeamMembers from './TeamMembers.svelte';
	import TeamLinks from './TeamLinks.svelte';
	import Thumbnail from './Thumbnail.svelte';
	import TeamInviteControls from './TeamInviteControls.svelte';
	let {
		team,
		onEdit,
	}: { team: TeamDirectoryItem; onEdit: (team: Team) => void } = $props();
	const session = useSession();
	const api = createTeamsApi();
	const canManage = $derived(
		session.signedIn &&
			team.members.some(
				(member) =>
					member.player.id === session.player?.id && member.role === 'admin',
			),
	);
	const isMember = $derived(
		session.signedIn &&
			team.members.some((member) => member.player.id === session.player?.id),
	);
	const isLastAdmin = $derived(
		canManage &&
			team.members.filter((member) => member.role === 'admin').length === 1,
	);
	let pending = $state(false);
	let error = $state('');
	let confirmDisband = $state(false);
	let confirmLeave = $state(false);
	let inviteOpen = $state(false);
	let inviteEnabled = $state(untrack(() => team.invite_enabled));

	async function changeActive() {
		if (pending) return;
		pending = true;
		error = '';
		let changed = false;
		try {
			if (team.is_active)
				await api.archive(String(team.id), session.me.csrf_token);
			else await api.restore(String(team.id), session.me.csrf_token);
			changed = true;
			confirmDisband = false;
			confirmLeave = false;
			await invalidateAll();
		} catch (cause) {
			error = changed
				? 'Team updated, but the list could not refresh. Please reload.'
				: teamErrorMessage(cause);
		} finally {
			pending = false;
		}
	}

	async function leave() {
		if (pending || !session.player || (team.is_active && isLastAdmin)) return;
		pending = true;
		error = '';
		let changed = false;
		try {
			await api.removeMember(team.id, session.player.id, session.me.csrf_token);
			changed = true;
			confirmLeave = false;
			await invalidateAll();
		} catch (cause) {
			error = changed
				? 'You left the team, but the list could not refresh. Please reload.'
				: teamErrorMessage(cause);
		} finally {
			pending = false;
		}
	}
</script>

<li id={`team-${team.id}`} class="min-w-0 scroll-mt-4">
	<article
		class="overflow-hidden rounded-lg border bg-card"
		aria-labelledby={`team-title-${team.id}`}
	>
		<div
			class="grid min-w-0 grid-cols-[6rem_minmax(0,1fr)] sm:grid-cols-[9rem_minmax(0,1fr)] xl:grid-cols-[11rem_minmax(0,1fr)]"
		>
			<div class="flex min-w-0 flex-col overflow-hidden">
				<div class="relative min-h-32 flex-1">
					<Thumbnail class="absolute inset-0 size-full aspect-auto" />
				</div>

				{#if teamWebsite(team.website_url) || teamWebsite(team.discord_url)}
					<div class="flex flex-wrap items-center justify-center gap-1 p-2">
						<TeamLinks {team} />
					</div>
				{/if}
			</div>
			<div class="min-w-0 flex-1 space-y-4 p-4 sm:p-6">
				<div class="flex items-start justify-between gap-2">
					<h2
						id={`team-title-${team.id}`}
						class="min-w-0 flex-1 break-words text-lg font-semibold"
					>
						{team.name}
					</h2>
					<div class="shrink-0">
						{#if isMember}
							<DropdownMenu.Root
								><DropdownMenu.Trigger
									>{#snippet child({ props })}<Button
											{...props}
											variant="ghost"
											size="icon-sm"
											disabled={pending || session.pending}
											aria-label={`Actions for ${team.name}`}
											><EllipsisIcon /></Button
										>{/snippet}</DropdownMenu.Trigger
								>
								<DropdownMenu.Content align="end">
									{#if canManage && team.is_active}<DropdownMenu.Item
											onSelect={() => onEdit(team)}>Edit team</DropdownMenu.Item
										><DropdownMenu.Item
											onSelect={() => {
												inviteOpen = true;
											}}>Invite link</DropdownMenu.Item
										>{/if}
									{#if canManage}{#if team.is_active}<DropdownMenu.Separator
											/><DropdownMenu.Item
												variant="destructive"
												onSelect={() => {
													error = '';
													confirmDisband = true;
												}}>Disband team</DropdownMenu.Item
											>{:else}<DropdownMenu.Item onSelect={changeActive}
												>Restore team</DropdownMenu.Item
											>{/if}{/if}
									{#if canManage}<DropdownMenu.Separator />{/if}
									<DropdownMenu.Item
										variant="destructive"
										onSelect={() => {
											error = '';
											confirmLeave = true;
										}}>Leave team</DropdownMenu.Item
									>
								</DropdownMenu.Content>
							</DropdownMenu.Root>
						{/if}
					</div>
				</div>
				{#if !team.is_active}<p class="text-sm text-muted-foreground">
						Disbanded
					</p>{/if}
				{#if team.description}<p
						class="whitespace-pre-wrap break-words text-sm text-muted-foreground"
					>
						{team.description}
					</p>{/if}
				{#if error && !confirmDisband && !confirmLeave}<p
						role="alert"
						class="text-sm text-destructive"
					>
						{error}
					</p>{/if}
				<TeamMembers {team} compact />
			</div>
		</div>
	</article>
</li>

{#if canManage && team.is_active}<Dialog.Root bind:open={inviteOpen}>
		<Dialog.Content
			><Dialog.Header
				><Dialog.Title>Invite members to {team.name}</Dialog.Title
				><Dialog.Description
					>Share a link so players can join this team.</Dialog.Description
				></Dialog.Header
			><TeamInviteControls
				team={String(team.id)}
				initiallyEnabled={inviteEnabled}
				onEnabledChange={(enabled) => {
					inviteEnabled = enabled;
				}}
				embedded
			/></Dialog.Content
		>
	</Dialog.Root>{/if}
{#if canManage}<Dialog.Root bind:open={confirmDisband}>
		<Dialog.Content
			><Dialog.Header
				><Dialog.Title>Disband {team.name}?</Dialog.Title><Dialog.Description
					>The team will be archived and its invite link revoked. Its details
					and memberships will be preserved. You can restore it later.</Dialog.Description
				></Dialog.Header
			>{#if error}<p role="alert" class="text-sm text-destructive">
					{error}
				</p>{/if}<Dialog.Footer
				><Button
					variant="outline"
					disabled={pending}
					onclick={() => {
						confirmDisband = false;
					}}>Cancel</Button
				><Button
					variant="destructive"
					disabled={pending || session.pending}
					onclick={changeActive}
					>{pending ? 'Disbanding...' : 'Disband team'}</Button
				></Dialog.Footer
			></Dialog.Content
		>
	</Dialog.Root>{/if}

{#if isMember}<Dialog.Root bind:open={confirmLeave}>
		<Dialog.Content>
			<Dialog.Header>
				<Dialog.Title>Leave {team.name}?</Dialog.Title>
				<Dialog.Description>
					{#if isLastAdmin && team.is_active}You are the last admin of this
						team. Grant admin access to another member before leaving, or
						disband the team.
					{:else}You will leave {team.name}. Rejoining requires a valid invite
						link.{/if}
				</Dialog.Description>
			</Dialog.Header>
			{#if error}<p role="alert" class="text-sm text-destructive">
					{error}
				</p>{/if}
			<Dialog.Footer>
				<Button
					variant="outline"
					disabled={pending}
					onclick={() => {
						confirmLeave = false;
					}}>Cancel</Button
				>
				{#if isLastAdmin && team.is_active}<Button
						variant="destructive"
						disabled={pending || session.pending}
						onclick={() => {
							confirmLeave = false;
							confirmDisband = true;
						}}>Disband team</Button
					>
				{:else}<Button
						variant="destructive"
						disabled={pending || session.pending}
						onclick={leave}>{pending ? 'Leaving...' : 'Leave team'}</Button
					>{/if}
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>{/if}
