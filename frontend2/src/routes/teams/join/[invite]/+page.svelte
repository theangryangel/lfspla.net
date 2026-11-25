<script lang="ts">
	import HelpButton from '#lib/components/app/HelpButton.svelte';
	import { goto } from '$app/navigation';
	import { apiErrorMessage, useApi } from '#lib/api.js';
	import { useSession } from '#lib/session.svelte.js';
	import Breadcrumbs from '#lib/components/app/Breadcrumbs.svelte';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import PageHeading from '#lib/components/app/PageHeading.svelte';
	import TeamTags from '#lib/components/app/TeamTags.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	const api = useApi();
	const session = useSession();
	let pending = $state(false);
	let error = $state('');
	let joined = $state(false);
	const signInUrl = $derived(
		`/auth/lfs?return_to=${encodeURIComponent(`/teams/join/${data.token}`)}`,
	);
	async function join() {
		if (pending || joined) return;
		pending = true;
		error = '';
		try {
			const team = await api.teams.acceptTeamInvite({
				token: data.token,
				'X-CSRF-Token': session.me.csrf_token,
			});
			joined = true;
			try {
				await goto(`/teams/${team.id}`, { invalidateAll: true });
			} catch {
				error = 'You joined the team. Use the link below to open it.';
			}
		} catch (cause) {
			error = apiErrorMessage(cause);
		} finally {
			pending = false;
		}
	}
</script>

<svelte:head><meta name="referrer" content="no-referrer" /></svelte:head>
<main id="main" class="w-full min-w-0 flex-1">
	<div
		class="mx-auto flex w-full max-w-screen-2xl flex-wrap items-center justify-between gap-3 px-4 py-4 md:px-6 md:py-3"
	>
		<Breadcrumbs items={data.breadcrumbs} />
		<HelpButton />
	</div>
	<PageIdentity label="Team invitation"
		><div class="space-y-3 py-2">
			<PageHeading title={data.team.name}
				>You’ve been invited to join this team.</PageHeading
			><TeamTags team={data.team} />
		</div></PageIdentity
	>
	<div class="mx-auto w-full max-w-screen-2xl space-y-5 p-4 md:p-6">
		{#if data.team.description}<p
				class="max-w-prose whitespace-pre-wrap break-words leading-relaxed text-muted-foreground"
			>
				{data.team.description}
			</p>{/if}
		<p class="text-sm text-muted-foreground">
			{data.team.member_count.toLocaleString()} members · Joining adds your LFS account
			to the team’s public roster.
		</p>
		{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
		<div class="flex flex-wrap items-center gap-3">
			{#if joined}<Button href="/teams/{data.team.id}">Open team</Button>
			{:else if session.signedIn}<Button onclick={join} disabled={pending}
					>{pending ? 'Joining...' : 'Join team'}</Button
				><span class="text-sm text-muted-foreground"
					>as {session.player?.lfs_username}</span
				>
			{:else}<Button href={signInUrl}>Sign in with LFS to join</Button>{/if}
			<Button href="/teams/{data.team.id}" variant="outline" disabled={pending}
				>View team</Button
			>
		</div>
	</div>
</main>
