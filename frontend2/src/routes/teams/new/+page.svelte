<script lang="ts">
	import HelpButton from '#lib/components/app/HelpButton.svelte';
	import { beforeNavigate, goto } from '$app/navigation';
	import { apiErrorMessage, useApi } from '#lib/api.js';
	import { useSession } from '#lib/session.svelte.js';
	import Breadcrumbs from '#lib/components/app/Breadcrumbs.svelte';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import PageHeading from '#lib/components/app/PageHeading.svelte';
	import TeamDisciplines from '#lib/components/app/TeamDisciplines.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { teamForm, teamDetails } from '#lib/teams.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	const api = useApi();
	const session = useSession();
	let values = $state(teamForm());
	let pending = $state(false);
	let error = $state('');
	let createdId = $state<number | null>(null);
	const dirty = $derived(
		createdId === null && JSON.stringify(values) !== JSON.stringify(teamForm()),
	);
	beforeNavigate((navigation) => {
		if (!dirty || pending) return;
		if (
			navigation.willUnload ||
			!window.confirm('Discard the team details you have entered?')
		)
			navigation.cancel();
	});
	async function create(event: SubmitEvent) {
		event.preventDefault();
		if (pending || createdId !== null) return;
		pending = true;
		error = '';
		try {
			const team = await api.teams.createTeam({
				body: teamDetails(values),
				'X-CSRF-Token': session.me.csrf_token,
			});
			createdId = team.id;
			try {
				await goto(`/teams/${team.id}`, { invalidateAll: true });
			} catch {
				error = 'Your team was created. Use the link below to open it.';
			}
		} catch (cause) {
			error = apiErrorMessage(cause);
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
		<HelpButton />
	</div>
	<PageIdentity label="Create team"
		><div class="py-2">
			<PageHeading title="Create a team"
				>Bring your drivers together. You’ll be the team’s owner.</PageHeading
			>
		</div></PageIdentity
	>
	<div class="mx-auto w-full max-w-screen-2xl p-4 md:p-6">
		{#if !session.signedIn}
			<div class="space-y-4">
				<p class="text-sm text-muted-foreground">
					Sign in with your LFS account to create and manage a team.
				</p>
				<Button href="/auth/lfs?return_to=%2Fteams%2Fnew"
					>Sign in with LFS</Button
				>
			</div>
		{:else}
			<form id="create-team" onsubmit={create} class="max-w-2xl space-y-5">
				{#if error}<p role="alert" class="text-sm text-destructive">
						{error}
					</p>{/if}
				{#if createdId !== null}<Button href="/teams/{createdId}"
						>Open your team</Button
					>{/if}
				<fieldset disabled={pending || createdId !== null} class="space-y-5">
					<div class="grid gap-4 sm:grid-cols-[minmax(0,1fr)_10rem]">
						<div class="space-y-2">
							<label for="new-team-name" class="text-sm font-medium"
								>Team name</label
							><Input
								id="new-team-name"
								bind:value={values.name}
								required
								maxlength={100}
								autocomplete="organization"
							/>
						</div>
						<div class="space-y-2">
							<label for="new-team-tag" class="text-sm font-medium"
								>Team tag</label
							><Input
								id="new-team-tag"
								bind:value={values.tag}
								maxlength={32}
								placeholder="Optional"
							/>
						</div>
					</div>
					<div class="space-y-2">
						<label for="new-team-description" class="text-sm font-medium"
							>Description</label
						><Textarea
							id="new-team-description"
							bind:value={values.description}
							maxlength={5000}
							rows={6}
						/>
					</div>
					<div class="space-y-2">
						<label for="new-team-website" class="text-sm font-medium"
							>Website</label
						><Input
							id="new-team-website"
							bind:value={values.website_url}
							maxlength={2048}
							placeholder="https://example.com"
						/>
					</div>
					<div class="space-y-2">
						<span class="text-sm font-medium">Disciplines</span><TeamDisciplines
							bind:value={values.tags}
							disabled={pending || createdId !== null}
						/>
					</div>
					<div class="flex items-start gap-3 border-t pt-4">
						<Checkbox
							id="new-team-applications"
							class="mt-0.5"
							bind:checked={values.accepts_applications}
							disabled={pending || createdId !== null}
							aria-describedby="new-team-applications-help"
						/>
						<div class="min-w-0 space-y-1">
							<label
								for="new-team-applications"
								class="cursor-pointer text-sm font-medium"
								>Open to applications</label
							>
							<p
								id="new-team-applications-help"
								class="text-sm text-muted-foreground"
							>
								Welcome enquiries. Players join through an invite link.
							</p>
						</div>
					</div>
				</fieldset>
				<div class="flex flex-wrap items-center gap-2">
					<Button type="submit" disabled={pending || createdId !== null}
						>{pending ? 'Creating...' : 'Create team'}</Button
					>
					<Button href="/teams" variant="outline" disabled={pending}
						>Cancel</Button
					>
				</div>
			</form>
		{/if}
	</div>
</main>
