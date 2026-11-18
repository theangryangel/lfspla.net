<script lang="ts">
	import { untrack } from 'svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { createTeamsApi, teamErrorMessage, type Team } from '#lib/teams.js';
	import { useSession } from '#lib/session.svelte.js';
	let {
		team,
		onSaved,
		onCancel,
	}: {
		team?: Team;
		onSaved: (team: Team) => Promise<void>;
		onCancel: () => void;
	} = $props();
	const session = useSession();
	const api = createTeamsApi();
	const id = $props.id();
	let name = $state(untrack(() => team?.name ?? ''));
	let description = $state(untrack(() => team?.description ?? ''));
	let website = $state(untrack(() => team?.website_url ?? ''));
	let discord = $state(untrack(() => team?.discord_url ?? ''));
	let pending = $state(false);
	let error = $state('');
	let savedTeam = $state<Team | null>(null);

	async function finish() {
		if (!savedTeam) return;
		try {
			await onSaved(savedTeam);
		} catch {
			error = 'Team saved, but the list could not refresh. Please try again.';
		}
	}
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (pending || savedTeam || !session.signedIn || !name.trim()) return;
		pending = true;
		error = '';
		try {
			savedTeam = team
				? await api.update(
						String(team.id),
						{
							name: name.trim(),
							description: description.trim() || null,
							website_url: website.trim() || null,
							discord_url: discord.trim() || null,
						},
						session.me.csrf_token,
					)
				: await api.create(
						{
							name: name.trim(),
							description: description.trim() || undefined,
							website_url: website.trim() || undefined,
							discord_url: discord.trim() || undefined,
						},
						session.me.csrf_token,
					);
			await finish();
		} catch (cause) {
			error = teamErrorMessage(cause);
		} finally {
			pending = false;
		}
	}
</script>

{#if savedTeam}
	<p role="status">{savedTeam.name} was saved.</p>
	{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
	<Button
		disabled={pending}
		onclick={async () => {
			pending = true;
			try {
				await finish();
			} finally {
				pending = false;
			}
		}}>Refresh teams</Button
	>
{:else}
	<form
		onsubmit={submit}
		class="space-y-4"
		aria-label={team ? 'Edit team' : 'Create team'}
	>
		<fieldset
			disabled={pending || session.pending}
			class="space-y-4 disabled:opacity-60"
		>
			<div class="space-y-2">
				<label for={`${id}-name`} class="text-sm font-medium">Name</label><Input
					id={`${id}-name`}
					name="name"
					required
					maxlength={100}
					bind:value={name}
				/>
			</div>
			<div class="space-y-2">
				<label for={`${id}-description`} class="text-sm font-medium"
					>Description</label
				><textarea
					id={`${id}-description`}
					name="description"
					rows="4"
					maxlength={5000}
					bind:value={description}
					class="w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
				></textarea>
			</div>
			<div class="space-y-2">
				<label for={`${id}-website`} class="text-sm font-medium"
					>Website URL</label
				><Input
					id={`${id}-website`}
					name="website_url"
					type="url"
					pattern="https?://.+"
					maxlength={2048}
					placeholder="https://example.com"
					bind:value={website}
				/>
			</div>
			<div class="space-y-2">
				<label for={`${id}-discord`} class="text-sm font-medium"
					>Discord URL</label
				><Input
					id={`${id}-discord`}
					name="discord_url"
					type="url"
					pattern="https?://.+"
					maxlength={2048}
					placeholder="https://discord.gg/…"
					bind:value={discord}
				/>
			</div>
			<div class="flex justify-end gap-2">
				<Button type="button" variant="outline" onclick={onCancel}
					>Cancel</Button
				><Button type="submit" disabled={pending || !name.trim()}
					>{pending
						? 'Saving...'
						: team
							? 'Save changes'
							: 'Create team'}</Button
				>
			</div>
		</fieldset>
		{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
	</form>
{/if}
