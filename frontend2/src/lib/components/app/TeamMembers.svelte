<script lang="ts">
	import { invalidateAll } from '$app/navigation';
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import Flag from './Flag.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import Empty from './Empty.svelte';
	import {
		createTeamsApi,
		teamErrorMessage,
		type TeamWithMembers,
		type TeamMember,
	} from '#lib/teams.js';
	import { useSession } from '#lib/session.svelte.js';
	let { team, compact = false }: { team: TeamWithMembers; compact?: boolean } =
		$props();
	const session = useSession();
	const api = createTeamsApi();
	const isAdmin = $derived(
		session.signedIn &&
			team.members.some(
				(member) =>
					member.player.id === session.player?.id && member.role === 'admin',
			),
	);
	let showAll = $state(false);
	let pending = $state(false);
	let error = $state('');
	let confirmOpen = $state(false);
	let removalTarget = $state<TeamMember | null>(null);
	const groups = $derived.by(() => {
		const alphabetical = [...team.members].sort(
			(a, b) =>
				a.player.display_name.localeCompare(b.player.display_name, undefined, {
					sensitivity: 'base',
				}) ||
				a.player.lfs_username.localeCompare(b.player.lfs_username) ||
				a.player.id - b.player.id,
		);
		return [
			{
				key: 'admins',
				label: 'Admins',
				members: alphabetical.filter((member) => member.role === 'admin'),
			},
			{
				key: 'members',
				label: 'Members',
				members: alphabetical.filter((member) => member.role === 'member'),
			},
		].filter((group) => group.members.length > 0);
	});
	const hasMore = $derived(groups.some((group) => group.members.length > 8));
	function confirmRemoval(member: TeamMember) {
		removalTarget = member;
		error = '';
		confirmOpen = true;
	}
	async function changeRole(member: TeamMember) {
		if (pending) return;
		pending = true;
		error = '';
		let changed = false;
		try {
			await api.changeRole(
				team.id,
				member.player.id,
				member.role === 'admin' ? 'member' : 'admin',
				session.me.csrf_token,
			);
			changed = true;
			await invalidateAll();
		} catch (cause) {
			error = changed
				? 'Role updated, but the roster could not refresh. Please reload.'
				: teamErrorMessage(cause);
		} finally {
			pending = false;
		}
	}
	async function apply() {
		if (!removalTarget || pending) return;
		const target = removalTarget;
		pending = true;
		error = '';
		let changed = false;
		try {
			await api.removeMember(team.id, target.player.id, session.me.csrf_token);
			changed = true;
			confirmOpen = false;
			await invalidateAll();
		} catch (cause) {
			error = changed
				? 'Membership updated, but the roster could not refresh. Please reload.'
				: teamErrorMessage(cause);
		} finally {
			pending = false;
		}
	}
</script>

<section
	aria-labelledby={compact ? undefined : `members-title-${team.id}`}
	aria-label={compact ? `${team.name} members` : undefined}
	aria-busy={pending}
	class="min-w-0 space-y-3"
>
	{#if !compact}<div class="flex items-center justify-between gap-3">
			<h3 id={`members-title-${team.id}`} class="racing-section-title">
				Members
			</h3>
		</div>
	{/if}
	{#if groups.length}
		{#each groups as group (group.key)}
			<div class="space-y-1">
				<h3
					id={`team-${team.id}-${group.key}`}
					class="flex items-center gap-1.5 text-xs font-medium text-muted-foreground"
				>
					{group.label}
				</h3>
				<ul
					class="flex flex-wrap gap-x-3 gap-y-1 text-sm"
					aria-labelledby={`team-${team.id}-${group.key}`}
				>
					{#each showAll ? group.members : group.members.slice(0, 8) as member (member.player.id)}
						<li
							class="member-row inline-flex max-w-full min-w-0 items-center gap-0.5 rounded-md py-0.5"
						>
							<div class="flex min-w-0 items-center gap-2">
								<a
									class="inline-flex min-w-0 items-center gap-1.5 font-medium hover:underline"
									href={`/drivers/${encodeURIComponent(member.player.lfs_username)}`}
									title={member.player.lfs_username}
								>
									<Flag
										code={member.player.flag_code}
										fallback={member.player.country_code}
									/>
									<span class="truncate">{member.player.display_name}</span>
								</a>
							</div>
							<div class="member-actions inline-flex shrink-0 items-center">
								{#if team.is_active && isAdmin}
									<DropdownMenu.Root
										><DropdownMenu.Trigger
											>{#snippet child({ props })}<Button
													{...props}
													size="icon-xs"
													variant="ghost"
													disabled={pending || session.pending}
													aria-label={`Actions for ${member.player.lfs_username}`}
													><EllipsisIcon /></Button
												>{/snippet}</DropdownMenu.Trigger
										>
										<DropdownMenu.Content align="end">
											{#if member.player.id === session.player?.id}<DropdownMenu.Item
													onSelect={() => changeRole(member)}
													>Revoke admin access</DropdownMenu.Item
												>
											{:else}
												{#if isAdmin && team.is_active}<DropdownMenu.Item
														onSelect={() => changeRole(member)}
														>{member.role === 'admin'
															? 'Revoke admin access'
															: 'Grant admin access'}</DropdownMenu.Item
													><DropdownMenu.Separator />{/if}
												<DropdownMenu.Item
													variant="destructive"
													onSelect={() => confirmRemoval(member)}
													>Remove from team</DropdownMenu.Item
												>
											{/if}
										</DropdownMenu.Content>
									</DropdownMenu.Root>
								{/if}
							</div>
						</li>
					{/each}
				</ul>
			</div>
		{/each}
	{:else}<Empty title="No members" />{/if}
	{#if hasMore}<Button
			variant="link"
			size="sm"
			class="h-auto p-0"
			aria-expanded={showAll}
			onclick={() => {
				showAll = !showAll;
			}}
			>{showAll
				? 'Show fewer members'
				: `Show all ${team.members.length} members`}</Button
		>{/if}
	{#if pending}<p role="status" class="text-sm text-muted-foreground">
			Updating membership...
		</p>{/if}
	{#if error && !confirmOpen}<p role="alert" class="text-sm text-destructive">
			{error}
		</p>{/if}
</section>

<Dialog.Root bind:open={confirmOpen}>
	<Dialog.Content>
		<Dialog.Header
			><Dialog.Title>Remove member?</Dialog.Title><Dialog.Description>
				{#if removalTarget}Remove {removalTarget.player.lfs_username} from {team.name}?
					They can rejoin with a valid invite link.{/if}
			</Dialog.Description></Dialog.Header
		>
		{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
		<Dialog.Footer
			><Button
				variant="outline"
				disabled={pending}
				onclick={() => {
					confirmOpen = false;
				}}>Cancel</Button
			><Button
				variant="destructive"
				disabled={pending || session.pending}
				onclick={apply}>{pending ? 'Updating...' : 'Remove member'}</Button
			></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>
