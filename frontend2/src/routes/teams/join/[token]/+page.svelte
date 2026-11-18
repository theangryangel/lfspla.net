<script lang="ts">
	import { invalidateAll } from '$app/navigation';
	import { page } from '$app/state';
	import TeamProfile from '#lib/components/app/TeamProfile.svelte';
	import Empty from '#lib/components/app/Empty.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { useSession } from '#lib/session.svelte.js';
	import {
		createTeamsApi,
		TeamApiError,
		teamErrorMessage,
	} from '#lib/teams.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	const session = useSession();
	const api = createTeamsApi();
	let pending = $state(false);
	let error = $state('');
	let joinedToken = $state('');
	let invalidToken = $state('');
	let errorToken = $state('');
	const signInUrl = $derived(
		`/auth/lfs?return_to=${encodeURIComponent(page.url.pathname + page.url.search)}`,
	);

	async function join() {
		if (pending || !session.signedIn) return;
		const token = data.token;
		pending = true;
		error = '';
		errorToken = token;
		try {
			await api.join(token, session.me.csrf_token);
			joinedToken = token;
		} catch (cause) {
			if (
				cause instanceof TeamApiError &&
				[400, 404, 410].includes(cause.status)
			) {
				invalidToken = token;
				error =
					'This invite link is no longer valid. Ask a team owner or admin for a new link.';
			} else {
				error = teamErrorMessage(cause);
			}
		} finally {
			pending = false;
		}
	}
</script>

<main
	id="main"
	class="mx-auto w-full max-w-screen-2xl flex-1 space-y-6 p-4 md:p-6"
>
	<div class="max-w-2xl space-y-6">
		{#if data.team}
			<TeamProfile team={data.team} />
			<section
				class="space-y-4 rounded-lg border p-4"
				aria-label="Team invitation"
			>
				{#if joinedToken === data.token}
					<p role="status">You are now a member of {data.team.name}.</p>
					<Button
						href={`/teams?q=${encodeURIComponent(data.team.name)}#team-${data.team.id}`}
						>Back to teams</Button
					>
				{:else if invalidToken !== data.token}
					<p>You have been invited to join {data.team.name} as a member.</p>
					{#if session.signedIn}
						<Button disabled={pending || session.pending} onclick={join}
							>{pending ? 'Joining...' : 'Join team'}</Button
						>
					{:else}
						<p class="text-sm text-muted-foreground">
							Sign in, then choose Join team to accept this invitation.
						</p>
						<Button href={signInUrl}>Sign in with LFS</Button>
					{/if}
				{/if}
				{#if error && errorToken === data.token}<p
						role="alert"
						class="text-sm text-destructive"
					>
						{error}
					</p>{/if}
			</section>
		{:else}
			<Empty
				title={data.invalid
					? 'Invite unavailable'
					: 'Could not load invitation'}
			>
				<p role="alert">{data.loadError}</p>
				{#if !data.invalid}<Button
						variant="outline"
						onclick={() => invalidateAll()}>Try again</Button
					>{/if}
				<a class="block text-sm underline" href="/teams">Browse teams</a>
			</Empty>
		{/if}
	</div>
</main>
