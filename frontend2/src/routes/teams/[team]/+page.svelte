<script lang="ts">
	import { beforeNavigate, invalidateAll } from '$app/navigation';
	import Globe from '@lucide/svelte/icons/globe';
	import Pencil from '@lucide/svelte/icons/pencil';
	import UserPlus from '@lucide/svelte/icons/user-plus';
	import MoreHorizontal from '@lucide/svelte/icons/ellipsis';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import {
		apiErrorMessage,
		useApi,
		type TeamMemberResponse,
	} from '#lib/api.js';
	import { useSession } from '#lib/session.svelte.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import HelpButton from '#lib/components/app/HelpButton.svelte';
	import Breadcrumbs from '#lib/components/app/Breadcrumbs.svelte';
	import Flag from '#lib/components/app/Flag.svelte';
	import Empty from '#lib/components/app/Empty.svelte';
	import TableFrame from '#lib/components/app/TableFrame.svelte';
	import TeamTags from '#lib/components/app/TeamTags.svelte';
	import TeamDisciplines from '#lib/components/app/TeamDisciplines.svelte';
	import TeamInvites from '#lib/components/app/TeamInvites.svelte';
	import ChoiceSelect from '#lib/components/app/ChoiceSelect.svelte';
	import {
		teamWebsite,
		teamForm,
		teamDetails,
		type TeamFormValues,
	} from '#lib/teams.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const api = useApi();
	const session = useSession();
	const team = $derived(data.team);
	const website = $derived(teamWebsite(team.website_url));
	const ownMembership = $derived(
		data.members.find((member) => member.player.id === session.player?.id),
	);
	const owner = $derived(ownMembership?.role === 'owner');
	const ownerCount = $derived(
		data.members.filter((member) => member.role === 'owner').length,
	);
	let draft = $state<TeamFormValues | null>(null);
	let pending = $state(false);
	let error = $state('');
	let notice = $state('');
	let inviteOpen = $state(false);
	let viewedTeamId: number | undefined;
	let confirmation = $state<
		| 'disband'
		| 'reactivate'
		| 'remove'
		| 'leave'
		| 'transfer'
		| 'promote'
		| 'demote'
		| null
	>(null);
	let target = $state<TeamMemberResponse | null>(null);
	let transferId = $state('');
	let confirmError = $state('');
	const dirty = $derived(
		draft !== null && JSON.stringify(draft) !== JSON.stringify(teamForm(team)),
	);
	const transferOptions = $derived(
		data.members
			.filter((member) => member.player.id !== session.player?.id)
			.map((member) => ({
				value: String(member.player.id),
				label: member.player.lfs_username,
			})),
	);
	const headers = () => ({ 'X-CSRF-Token': session.me.csrf_token });

	$effect(() => {
		const id = team.id;
		if (viewedTeamId !== id) {
			viewedTeamId = id;
			draft = null;
			inviteOpen = false;
			confirmation = null;
			error = '';
			notice = '';
		}
	});
	$effect(() => {
		if (!owner) {
			draft = null;
			inviteOpen = false;
		}
	});
	beforeNavigate((navigation) => {
		if (!dirty || pending) return;
		if (
			navigation.willUnload ||
			!window.confirm('Leave this page and discard your unsaved team changes?')
		)
			navigation.cancel();
	});
	function edit() {
		draft = teamForm(team);
		error = '';
		notice = '';
	}
	function cancel() {
		draft = null;
		error = '';
	}
	async function refresh(message: string) {
		notice = message;
		try {
			await invalidateAll();
		} catch {
			error = `${message} Reload the page to see the updated team.`;
		}
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!draft || pending) return;
		pending = true;
		error = '';
		notice = '';
		try {
			await api.teams.updateTeam({
				id: team.id,
				...headers(),
				body: teamDetails(draft),
			});
			draft = null;
			await refresh('Team details saved.');
		} catch (cause) {
			error = apiErrorMessage(cause);
		} finally {
			pending = false;
		}
	}
	function confirm(
		action: typeof confirmation,
		member: TeamMemberResponse | null = null,
	) {
		confirmation = action;
		target = member;
		confirmError = '';
		transferId = member ? String(member.player.id) : '';
	}
	async function perform() {
		if (pending || !confirmation) return;
		pending = true;
		confirmError = '';
		error = '';
		notice = '';
		try {
			let message = '';
			if (confirmation === 'disband' || confirmation === 'reactivate') {
				const status = confirmation === 'disband' ? 'disbanded' : 'active';
				await api.teams.setTeamStatus({ id: team.id, ...headers(), status });
				message =
					status === 'active'
						? 'Team reactivated. Applications remain closed.'
						: 'Team disbanded. Invite links have been revoked.';
			} else if (confirmation === 'promote' || confirmation === 'demote') {
				if (!target) return;
				const role = confirmation === 'promote' ? 'owner' : 'member';
				await api.teams.setTeamMemberRole({
					id: team.id,
					player_id: target.player.id,
					...headers(),
					role,
				});
				message =
					role === 'owner'
						? `${target.player.lfs_username} is now an owner.`
						: `${target.player.lfs_username} is now a member. Invite links have been revoked.`;
			} else if (confirmation === 'transfer') {
				if (!transferId) return;
				await api.teams.transferTeamOwnership({
					id: team.id,
					...headers(),
					player_id: Number(transferId),
				});
				message =
					'Ownership transferred. You are now a member. Invite links have been revoked.';
			} else {
				const playerId =
					confirmation === 'leave' ? session.player?.id : target?.player.id;
				if (!playerId) return;
				await api.teams.removeTeamMember({
					id: team.id,
					player_id: playerId,
					...headers(),
				});
				message =
					confirmation === 'leave' ? 'You left the team.' : 'Member removed.';
			}
			confirmation = null;
			await refresh(message);
		} catch (cause) {
			confirmError = apiErrorMessage(cause);
		} finally {
			pending = false;
		}
	}
</script>

<main id="main" class="w-full min-w-0 flex-1">
	<div
		class="mx-auto flex w-full max-w-screen-2xl flex-wrap items-center justify-between gap-3 px-4 py-4 md:px-6 md:py-3"
	>
		<Breadcrumbs items={data.breadcrumbs} />
		<div class="flex flex-wrap items-center gap-2">
			<HelpButton />
			{#if owner}
				<div
					class="flex flex-wrap items-center gap-2"
					aria-label="Team management"
				>
					{#if draft}
						<Button variant="outline" disabled={pending} onclick={cancel}
							>Cancel</Button
						>
						<Button type="submit" form="team-edit" disabled={pending || !dirty}
							>{pending ? 'Saving...' : 'Save changes'}</Button
						>
					{:else}
						<Button variant="outline" disabled={pending} onclick={edit}
							><Pencil aria-hidden="true" />Edit team</Button
						>
						<Button
							variant="outline"
							disabled={pending || team.status !== 'active'}
							onclick={() => (inviteOpen = true)}
							><UserPlus aria-hidden="true" />Invite members</Button
						>
						<DropdownMenu.Root>
							<DropdownMenu.Trigger
								>{#snippet child({ props })}<Button
										{...props}
										variant="ghost"
										disabled={pending}
										>Manage team<ChevronDown aria-hidden="true" /></Button
									>{/snippet}</DropdownMenu.Trigger
							>
							<DropdownMenu.Content align="end">
								<DropdownMenu.Item
									disabled={!transferOptions.length}
									onSelect={() => confirm('transfer')}
									>Transfer ownership</DropdownMenu.Item
								>
								<DropdownMenu.Separator />
								{#if team.status === 'active'}<DropdownMenu.Item
										variant="destructive"
										onSelect={() => confirm('disband')}
										>Disband team</DropdownMenu.Item
									>
								{:else}<DropdownMenu.Item onSelect={() => confirm('reactivate')}
										>Reactivate team</DropdownMenu.Item
									>{/if}
							</DropdownMenu.Content>
						</DropdownMenu.Root>
					{/if}
				</div>
			{/if}
		</div>
	</div>
	<form id="team-edit" onsubmit={save}>
		<PageIdentity label="Team identity">
			<div
				class="flex min-w-0 flex-col gap-4 py-2 sm:flex-row sm:items-center sm:justify-between"
			>
				<div class="min-w-0 flex-1 space-y-3">
					{#if draft}
						<fieldset
							disabled={pending}
							class="grid max-w-2xl gap-4 sm:grid-cols-[minmax(0,1fr)_10rem]"
						>
							<div class="space-y-2">
								<label for="team-name" class="text-sm font-medium"
									>Team name</label
								><Input
									id="team-name"
									bind:value={draft.name}
									required
									maxlength={100}
								/>
							</div>
							<div class="space-y-2">
								<label for="team-tag" class="text-sm font-medium"
									>Team tag</label
								><Input
									id="team-tag"
									bind:value={draft.tag}
									maxlength={32}
									placeholder="Optional"
								/>
							</div>
						</fieldset>
					{:else}
						{#if team.tag}<p
								class="break-all font-mono text-sm text-muted-foreground"
							>
								{team.tag}
							</p>{/if}
						<h1 class="racing-page-title break-words">{team.name}</h1>
						<div
							class="flex flex-wrap items-center gap-x-4 gap-y-2 text-sm text-muted-foreground"
						>
							<span
								>{team.member_count.toLocaleString()}
								{team.member_count === 1 ? 'member' : 'members'}</span
							><TeamTags {team} />
						</div>
					{/if}
				</div>
				{#if website && !draft}<Button
						href={website}
						target="_blank"
						rel="noopener noreferrer"
						variant="outline"
						class="w-fit shrink-0"
						><Globe aria-hidden="true" />Team website<span class="sr-only">
							(opens in a new tab)</span
						></Button
					>{/if}
			</div>
		</PageIdentity>
		<div class="mx-auto w-full max-w-screen-2xl space-y-4 px-4 pt-4 md:px-6">
			{#if error}<p role="alert" class="text-sm text-destructive">
					{error}
				</p>{/if}
			{#if notice}<p role="status" class="text-sm text-muted-foreground">
					{notice}
				</p>{/if}
			{#if draft}<p class="text-sm text-muted-foreground">
					Editing team details. Save your changes when you’re done.
				</p>{/if}
		</div>
		<div
			class="mx-auto grid w-full max-w-screen-2xl gap-8 p-4 md:p-6 lg:grid-cols-[minmax(0,1fr)_20rem] lg:gap-12"
		>
			<div class="min-w-0 space-y-8">
				{#if team.description || draft}
					<section aria-labelledby="about-team" class="space-y-3">
						<h2 id="about-team" class="text-lg font-semibold">
							About the team
						</h2>
						{#if draft}
							<fieldset disabled={pending} class="max-w-2xl space-y-4">
								<div class="space-y-2">
									<label for="team-description" class="text-sm font-medium"
										>Description</label
									><Textarea
										id="team-description"
										bind:value={draft.description}
										maxlength={5000}
										rows={6}
									/>
								</div>
								<div class="space-y-2">
									<label for="team-website" class="text-sm font-medium"
										>Website</label
									><Input
										id="team-website"
										bind:value={draft.website_url}
										maxlength={2048}
										placeholder="https://example.com"
									/>
								</div>
								<div class="space-y-2">
									<span class="text-sm font-medium">Disciplines</span
									><TeamDisciplines
										bind:value={draft.tags}
										disabled={pending}
									/>
								</div>
							</fieldset>
						{:else}<p
								class="max-w-prose whitespace-pre-wrap break-words leading-relaxed text-muted-foreground"
							>
								{team.description}
							</p>{/if}
					</section>
				{/if}
				<section aria-labelledby="team-roster" class="space-y-3">
					<h2 id="team-roster" class="text-lg font-semibold">
						Members <span
							class="ml-1 text-sm font-normal tabular-nums text-muted-foreground"
							>{team.member_count.toLocaleString()}</span
						>
					</h2>
					{#if data.members.length}
						<TableFrame appearance="data">
							<Table.Root
								class="[&_th:first-child]:pl-4 [&_td:first-child]:pl-4 [&_th:last-child]:pr-4 [&_td:last-child]:pr-4"
							>
								<Table.Header
									><Table.Row
										><Table.Head>Driver</Table.Head><Table.Head>Role</Table.Head
										>{#if ownMembership}<Table.Head class="w-12"
												><span class="sr-only">Member actions</span></Table.Head
											>{/if}</Table.Row
									></Table.Header
								>
								<Table.Body>
									{#each data.members as member (member.player.id)}
										<Table.Row>
											<Table.Cell class="whitespace-normal py-3"
												><a
													href="/drivers/{encodeURIComponent(
														member.player.lfs_username,
													)}"
													class="group flex min-w-0 items-center gap-3 rounded-sm focus-visible:outline-2 focus-visible:outline-ring"
												>
													{#if member.player.country_code || member.player.flag_code}<Flag
															code={member.player.flag_code}
															fallback={member.player.country_code}
														/>{/if}
													<span class="min-w-0"
														><span
															class="block break-words font-medium group-hover:underline"
															>{member.player.display_name}</span
														>{#if member.player.display_name !== member.player.lfs_username}<span
																class="block break-words text-muted-foreground"
																>{member.player.lfs_username}</span
															>{/if}</span
													>
												</a></Table.Cell
											>
											<Table.Cell
												>{#if member.role === 'owner'}<Badge variant="outline"
														>Owner</Badge
													>{:else}<span class="text-muted-foreground"
														>Member</span
													>{/if}</Table.Cell
											>
											{#if ownMembership}<Table.Cell class="text-right">
													{#if owner || member.player.id === session.player?.id}
														<DropdownMenu.Root>
															<DropdownMenu.Trigger
																>{#snippet child({ props })}<Button
																		{...props}
																		variant="ghost"
																		size="icon-sm"
																		aria-label={`Actions for ${member.player.lfs_username}`}
																		disabled={pending || draft !== null}
																		><MoreHorizontal
																			aria-hidden="true"
																		/></Button
																	>{/snippet}</DropdownMenu.Trigger
															>
															<DropdownMenu.Content align="end">
																{#if member.player.id === session.player?.id}
																	{#if owner && ownerCount > 1}<DropdownMenu.Item
																			onSelect={() => confirm('demote', member)}
																			>Make member</DropdownMenu.Item
																		><DropdownMenu.Separator />{/if}
																	{#if member.role === 'owner' && ownerCount <= 1}<DropdownMenu.Item
																			disabled
																			>Make another member an owner before
																			leaving</DropdownMenu.Item
																		>
																	{:else}<DropdownMenu.Item
																			variant="destructive"
																			onSelect={() => confirm('leave')}
																			>Leave team</DropdownMenu.Item
																		>{/if}
																{:else if owner}
																	{#if member.role === 'member'}<DropdownMenu.Item
																			onSelect={() =>
																				confirm('promote', member)}
																			>Make owner</DropdownMenu.Item
																		>
																	{:else}<DropdownMenu.Item
																			onSelect={() => confirm('demote', member)}
																			>Make member</DropdownMenu.Item
																		>{/if}

																	<DropdownMenu.Item
																		onSelect={() => confirm('transfer', member)}
																		>Transfer ownership</DropdownMenu.Item
																	>
																	<DropdownMenu.Separator />
																	<DropdownMenu.Item
																		variant="destructive"
																		onSelect={() => confirm('remove', member)}
																		>Remove member</DropdownMenu.Item
																	>
																{/if}
															</DropdownMenu.Content>
														</DropdownMenu.Root>
													{/if}
												</Table.Cell>{/if}
										</Table.Row>
									{/each}
								</Table.Body>
							</Table.Root>
						</TableFrame>
					{:else}<Empty title="No members listed" />{/if}
				</section>
			</div>
			<aside
				class="space-y-3 border-t pt-5 lg:border-t-0 lg:border-l lg:pt-0 lg:pl-6"
				aria-labelledby="team-applications"
			>
				<h2 id="team-applications" class="text-base font-semibold">
					Applications
				</h2>
				{#if draft}
					<div class="flex items-start gap-3">
						<Checkbox
							id="team-applications-open"
							class="mt-0.5"
							bind:checked={draft.accepts_applications}
							disabled={pending || team.status === 'disbanded'}
							aria-describedby="team-applications-help"
						/>
						<div class="min-w-0 space-y-1">
							<label
								for="team-applications-open"
								class="cursor-pointer text-sm font-medium"
								>Open to applications</label
							>
							<p
								id="team-applications-help"
								class="text-sm leading-relaxed text-muted-foreground"
							>
								{team.status === 'disbanded'
									? 'Reactivate the team before opening applications.'
									: 'Show that your team welcomes enquiries. People still join through an invite link.'}
							</p>
						</div>
					</div>
				{:else}
					<p class="text-sm font-medium">
						{team.accepts_applications && team.status === 'active'
							? 'Open to applications'
							: 'Applications closed'}
					</p>
					<p class="text-sm leading-relaxed text-muted-foreground">
						{team.status === 'disbanded'
							? 'This team has disbanded.'
							: team.accepts_applications
								? 'Interested in joining? Contact the team to find out more.'
								: 'This team is not currently accepting applications.'}
					</p>
					{#if website && team.status === 'active' && team.accepts_applications}<a
							href={website}
							target="_blank"
							rel="noopener noreferrer"
							class="inline-block text-sm text-primary underline-offset-4 hover:underline"
							>Visit the team website<span class="sr-only">
								(opens in a new tab)</span
							></a
						>{/if}
				{/if}
			</aside>
		</div>
	</form>
</main>

{#if owner}<TeamInvites teamId={team.id} bind:open={inviteOpen} />{/if}
<Dialog.Root
	open={confirmation !== null}
	onOpenChange={(open) => {
		if (!open && !pending) confirmation = null;
	}}
>
	<Dialog.Content showCloseButton={!pending}>
		<Dialog.Header>
			<Dialog.Title
				>{confirmation === 'promote'
					? 'Make owner?'
					: confirmation === 'demote'
						? 'Make member?'
						: confirmation === 'disband'
							? 'Disband team?'
							: confirmation === 'reactivate'
								? 'Reactivate team?'
								: confirmation === 'transfer'
									? 'Transfer ownership?'
									: confirmation === 'leave'
										? 'Leave team?'
										: 'Remove member?'}</Dialog.Title
			>
			<Dialog.Description
				>{confirmation === 'promote'
					? `${target?.player.lfs_username ?? 'This member'} will have full team management access alongside the existing owners.`
					: confirmation === 'demote'
						? `${target?.player.lfs_username ?? 'This owner'} will remain a member and lose management access. Existing invite links will be revoked.`
						: confirmation === 'disband'
							? 'The team page and roster will remain. Applications will close and all invite links will be revoked.'
							: confirmation === 'reactivate'
								? 'The team will become active again. Applications stay closed and old invite links remain revoked.'
								: confirmation === 'transfer'
									? 'The selected member becomes an owner. You become a member and lose management access. Existing invite links will be revoked.'
									: confirmation === 'leave'
										? 'You will leave this team. You will need a new invitation to rejoin.'
										: `Remove ${target?.player.lfs_username ?? 'this member'} from the team? They will need a new invitation to rejoin.`}</Dialog.Description
			>
		</Dialog.Header>
		{#if confirmation === 'transfer'}<ChoiceSelect
				label="New team owner"
				value={transferId}
				options={[{ value: '', label: 'Choose a member' }, ...transferOptions]}
				disabled={pending}
				onValueChange={(value) => (transferId = value)}
			/>{/if}
		{#if confirmError}<p role="alert" class="text-sm text-destructive">
				{confirmError}
			</p>{/if}
		<Dialog.Footer>
			<Button
				variant="outline"
				disabled={pending}
				onclick={() => (confirmation = null)}>Cancel</Button
			>
			<Button
				variant={confirmation === 'reactivate' ||
				confirmation === 'transfer' ||
				confirmation === 'promote'
					? 'default'
					: 'destructive'}
				disabled={pending || (confirmation === 'transfer' && !transferId)}
				onclick={perform}
				>{pending
					? 'Saving...'
					: confirmation === 'promote'
						? 'Make owner'
						: confirmation === 'demote'
							? 'Make member'
							: confirmation === 'disband'
								? 'Disband team'
								: confirmation === 'reactivate'
									? 'Reactivate team'
									: confirmation === 'transfer'
										? 'Transfer ownership'
										: confirmation === 'leave'
											? 'Leave team'
											: 'Remove member'}</Button
			>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
