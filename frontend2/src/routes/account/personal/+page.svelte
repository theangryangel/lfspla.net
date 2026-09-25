<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { send } from '$lib/api.js';
	import { useSession } from '$lib/session.svelte.js';
	import Panel from '$lib/components/app/Panel.svelte';
	import SearchSelect from '$lib/components/app/SearchSelect.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const session = useSession();
	let country = $derived(session.player?.country_code ?? '');
	let pending = $state(false);
	let error = $state('');
	let saved = $state(false);
	const options = $derived([
		{ value: '', label: 'Not specified' },
		...data.countries.map((c) => ({ value: c.code, label: c.name })),
	]);

	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		pending = true;
		error = '';
		saved = false;
		try {
			await send('/api/v1/me', {
				method: 'PATCH',
				csrf: session.me.csrf_token,
				json: { country_code: country || null },
			});
			saved = true;
			try {
				await invalidate('/api/v1/me');
			} catch {
				error =
					'Your settings were saved, but the page could not refresh. Reload to see the updated account.';
			}
		} catch (cause) {
			error =
				cause instanceof Error
					? cause.message
					: 'Your settings could not be saved.';
		} finally {
			pending = false;
		}
	}
</script>

<svelte:head><title>Personal settings · lfspla.net</title></svelte:head>
<p class="text-sm text-muted-foreground">
	Manage the preferences for your LFS account.
</p>
<div class="max-w-xl">
	<Panel title="Account preferences">
		<p class="text-sm text-muted-foreground">
			Signed in as <span class="font-medium text-foreground"
				>{session.player?.lfs_username}</span
			>.
		</p>
		<form onsubmit={save} class="space-y-4">
			<fieldset disabled={pending} class="space-y-2">
				<legend class="mb-2 text-sm font-medium">Country</legend>
				<SearchSelect
					label="Country"
					value={country}
					{options}
					placeholder="Search countries..."
					onValueChange={(value) => {
						country = value;
						saved = false;
					}}
				/>
				<p class="text-sm text-muted-foreground">
					Your country appears on your driver profile and in nation rankings.
				</p>
			</fieldset>
			{#if error}<p role="alert" class="text-sm text-destructive">
					{error}
				</p>{/if}
			{#if saved}<p role="status" class="text-sm">
					Your preferences have been saved.
				</p>{/if}
			<Button
				type="submit"
				disabled={pending || country === (session.player?.country_code ?? '')}
				>{pending ? 'Saving...' : 'Save preferences'}</Button
			>
		</form>
	</Panel>
</div>
