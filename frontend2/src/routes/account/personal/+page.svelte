<script lang="ts">
	import { apiErrorMessage, useApi, type CodeNameSummary } from '#lib/api.js';
	import * as InputGroup from '#lib/components/ui/input-group/index.js';
	import BrandIcon from '#lib/components/app/BrandIcon.svelte';
	import Globe from '@lucide/svelte/icons/globe';
	import { Button } from '#lib/components/ui/button/index.js';

	import { invalidate } from '$app/navigation';

	import { useSession } from '#lib/session.svelte.js';
	import Flag from '#lib/components/app/Flag.svelte';
	import SearchSelect from '#lib/components/app/SearchSelect.svelte';

	import type { PageProps } from './$types';

	const api = useApi();

	let { data }: PageProps = $props();
	const session = useSession();
	let country = $derived(session.player?.country_code ?? '');
	let flag = $derived(session.player?.flag_code ?? '');
	let twitch = $derived(session.me.profile_links?.twitch_channel ?? '');
	let youtube = $derived(session.me.profile_links?.youtube_channel ?? '');
	let video = $derived(
		session.me.profile_links?.youtube_video_id
			? `https://www.youtube.com/watch?v=${session.me.profile_links.youtube_video_id}`
			: '',
	);
	let website = $derived(session.me.profile_links?.website_url ?? '');
	const linksChanged = $derived(
		twitch !== (session.me.profile_links?.twitch_channel ?? '') ||
			youtube !== (session.me.profile_links?.youtube_channel ?? '') ||
			website !== (session.me.profile_links?.website_url ?? '') ||
			video !==
				(session.me.profile_links?.youtube_video_id
					? `https://www.youtube.com/watch?v=${session.me.profile_links.youtube_video_id}`
					: ''),
	);

	function twitchChannel(value: string): string | null {
		const trimmed = value.trim();
		if (!trimmed) return null;
		if (/^[a-zA-Z0-9_]+$/.test(trimmed)) return trimmed;
		try {
			const url = new URL(trimmed);
			if (
				url.protocol === 'https:' &&
				['twitch.tv', 'www.twitch.tv'].includes(url.hostname) &&
				/^\/[a-zA-Z0-9_]+\/?$/.test(url.pathname)
			)
				return url.pathname.split('/')[1];
		} catch {
			/* Show the same field-specific error for malformed links. */
		}
		throw new Error('Enter a Twitch channel name or channel link.');
	}

	function youtubeVideo(value: string): string | null {
		const trimmed = value.trim();
		if (!trimmed) return null;
		if (/^[a-zA-Z0-9_-]{11}$/.test(trimmed)) return trimmed;
		try {
			const url = new URL(trimmed);
			let id: string | null = null;
			if (url.protocol === 'https:') {
				if (url.hostname === 'youtu.be') id = url.pathname.slice(1);
				if (
					['youtube.com', 'www.youtube.com', 'm.youtube.com'].includes(
						url.hostname,
					)
				) {
					if (url.pathname === '/watch') id = url.searchParams.get('v');
					else if (/^\/(shorts|embed|live)\//.test(url.pathname))
						id = url.pathname.split('/')[2];
				}
			}
			if (id && /^[a-zA-Z0-9_-]{11}$/.test(id)) return id;
		} catch {
			/* Show the same field-specific error for malformed links. */
		}
		throw new Error('Enter a valid YouTube video link.');
	}
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
				profile_links: {
					twitch_channel: twitchChannel(twitch),
					youtube_channel: youtube.trim() || null,
					youtube_video_id: youtubeVideo(video),
					website_url: website.trim() || null,
				},
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

<div class="racing-data max-w-xl space-y-4">
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
		<div class="space-y-4" oninput={() => (saved = false)}>
			<div class="space-y-2">
				<label for="profile-twitch" class="text-sm font-medium"
					>Twitch channel</label
				>
				<InputGroup.Root>
					<InputGroup.Addon
						class="border-r px-2.5 text-purple-700 dark:text-purple-300"
						aria-hidden="true"><BrandIcon name="twitch" /></InputGroup.Addon
					>
					<InputGroup.Input
						disabled={pending}
						id="profile-twitch"
						bind:value={twitch}
						placeholder="Channel name or https://www.twitch.tv/..."
						maxlength={2048}
					/>
				</InputGroup.Root>
			</div>
			<div class="space-y-2">
				<label for="profile-youtube" class="text-sm font-medium"
					>YouTube channel</label
				>
				<InputGroup.Root>
					<InputGroup.Addon
						class="border-r px-2.5 text-red-700 dark:text-red-300"
						aria-hidden="true"><BrandIcon name="youtube" /></InputGroup.Addon
					>
					<InputGroup.Input
						disabled={pending}
						id="profile-youtube"
						type="url"
						bind:value={youtube}
						placeholder="https://www.youtube.com/@yourchannel"
						maxlength={2048}
					/>
				</InputGroup.Root>
			</div>
			<div class="space-y-2">
				<label for="profile-video" class="text-sm font-medium"
					>Featured YouTube video</label
				>
				<InputGroup.Root>
					<InputGroup.Addon
						class="border-r px-2.5 text-red-700 dark:text-red-300"
						aria-hidden="true"><BrandIcon name="youtube" /></InputGroup.Addon
					>
					<InputGroup.Input
						disabled={pending}
						id="profile-video"
						bind:value={video}
						placeholder="https://www.youtube.com/watch?v=..."
						maxlength={2048}
						aria-describedby="profile-video-help"
					/>
				</InputGroup.Root>
				<p id="profile-video-help" class="text-sm text-muted-foreground">
					Visitors can watch this video in a dialog on your profile. Choose a
					video that allows embedding.
				</p>
			</div>
			<div class="space-y-2">
				<label for="profile-website" class="text-sm font-medium">Website</label>
				<InputGroup.Root>
					<InputGroup.Addon
						class="border-r px-2.5 text-muted-foreground"
						aria-hidden="true"><Globe class="size-4" /></InputGroup.Addon
					>
					<InputGroup.Input
						disabled={pending}
						id="profile-website"
						type="url"
						bind:value={website}
						placeholder="https://..."
						maxlength={2048}
					/>
				</InputGroup.Root>
			</div>
		</div>
		{#if error}<p role="alert" class="text-sm text-destructive">
				{error}
			</p>{/if}
		{#if saved}<p role="status" class="text-sm">
				Your preferences have been saved.
			</p>{/if}
		<div class="flex flex-wrap items-center justify-between gap-3">
			<Button
				type="submit"
				disabled={pending ||
					loadingFlags ||
					!!flagError ||
					(country === (session.player?.country_code ?? '') &&
						flag === (session.player?.flag_code ?? '') &&
						!linksChanged)}>{pending ? 'Saving...' : 'Save preferences'}</Button
			>
			{#if session.player}
				<a
					class="text-sm text-muted-foreground underline underline-offset-4 hover:text-foreground"
					href={`/drivers/${encodeURIComponent(session.player.lfs_username)}`}
					>View your profile</a
				>
			{/if}
		</div>
	</form>
</div>
