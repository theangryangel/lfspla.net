<script lang="ts">
	import { apiErrorMessage, useApi, type CodeNameSummary } from '$lib/api.js';
	import { Button } from '$lib/components/ui/button/index.js';

	import { invalidate } from '$app/navigation';

	import { useSession } from '$lib/session.svelte.js';
	import Flag from '$lib/components/app/Flag.svelte';
	import Panel from '$lib/components/app/Panel.svelte';
	import SearchSelect from '$lib/components/app/SearchSelect.svelte';

	import type { PageProps } from './$types';

	const api = useApi();

	let { data }: PageProps = $props();
	const session = useSession();
	let country = $derived(session.player?.country_code ?? '');
	let flag = $derived(session.player?.flag_code ?? '');
	let flags = $state<CodeNameSummary[]>([]);
	let loadingFlags = $state(false);
	let flagError = $state('');
	let retry = $state(0);
	$effect(() => {
		const code = country;
		retry;
		let cancelled = false;
		flags = [];
		flagError = '';
		loadingFlags = !!code;
		if (code) {
			api.countries
				.listCountryFlags({ code })
				.then((response) => response.items)
				.then((choices) => {
					if (!cancelled) flags = choices;
				})
				.catch(() => {
					if (!cancelled) flagError = 'Could not load flags.';
				})
				.finally(() => {
					if (!cancelled) loadingFlags = false;
				});
		}
		return () => {
			cancelled = true;
		};
	});
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
			await api.authentication.update({
				'X-CSRF-Token': session.me.csrf_token,
				country_code: country || null,
				flag_code: flag || null,
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
					? apiErrorMessage(cause)
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
						flag = '';
						saved = false;
					}}
				/>
				<p class="text-sm text-muted-foreground">
					Your country appears on your driver profile and in nation rankings.
				</p>
			</fieldset>
			{#if country}
				<fieldset
					disabled={pending || loadingFlags || !!flagError}
					class="space-y-2"
				>
					<legend class="mb-2 text-sm font-medium">Display flag</legend>
					<SearchSelect
						label="Display flag"
						value={flag}
						options={[
							{ value: '', label: 'Country flag (default)' },
							...flags.map((choice) => ({
								value: choice.code,
								label: choice.name,
							})),
						]}
						placeholder="Search flags..."
						onValueChange={(value) => {
							flag = value;
							saved = false;
						}}
					/>
					<p class="text-sm text-muted-foreground">
						<Flag code={flag} fallback={country} /> Shown beside your name.
					</p>
				</fieldset>
				{#if loadingFlags}<p role="status" class="text-sm">
						Loading flags...
					</p>{/if}
				{#if flagError}<p role="alert" class="text-sm text-destructive">
						{flagError}
					</p>
					<Button type="button" onclick={() => retry++}>Retry</Button>
				{/if}
			{/if}
			{#if error}<p role="alert" class="text-sm text-destructive">
					{error}
				</p>{/if}
			{#if saved}<p role="status" class="text-sm">
					Your preferences have been saved.
				</p>{/if}
			<Button
				type="submit"
				disabled={pending ||
					loadingFlags ||
					!!flagError ||
					(country === (session.player?.country_code ?? '') &&
						flag === (session.player?.flag_code ?? ''))}
				>{pending ? 'Saving...' : 'Save preferences'}</Button
			>
		</form>
	</Panel>
</div>
