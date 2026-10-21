<script lang="ts">
	import { navigating, page } from '$app/state';
	import { beforeNavigate } from '$app/navigation';
	import { onDestroy } from 'svelte';
	import CompareButton from '#lib/components/app/CompareButton.svelte';
	import { Input } from '#lib/components/ui/input/index.js';
	import SearchIcon from '@lucide/svelte/icons/search';
	import Empty from '#lib/components/app/Empty.svelte';
	import Flag from '#lib/components/app/Flag.svelte';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import PageHeading from '#lib/components/app/PageHeading.svelte';
	import { setQuery } from '#lib/query.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let draft = $state<string | null>(null);
	const search = $derived(draft ?? page.url.searchParams.get('q') ?? '');
	const searches = new Set<string>();
	let searchError = $state('');
	let pending = $state(false);
	let latest = 0;
	const searching = $derived(
		pending || navigating.to?.url.pathname === '/drivers',
	);
	let timer: ReturnType<typeof setTimeout> | undefined;
	const cancelSearch = () => clearTimeout(timer);

	beforeNavigate(({ type, to, shallow }) => {
		if (shallow && type === 'goto') return;
		if (
			type === 'goto' &&
			to?.url.pathname === '/drivers' &&
			searches.has(to.url.searchParams.get('q') ?? '')
		)
			return;

		searches.clear();
		cancelSearch();
		draft = null;
		pending = false;
		latest++;
	});
	onDestroy(cancelSearch);

	function searchDrivers(value: string) {
		draft = value;
		searchError = '';
		cancelSearch();
		pending = true;
		const token = ++latest;
		timer = setTimeout(() => {
			// Keep newer typing local while an older search is loading.
			searches.add(value);
			void setQuery({ q: value, page: '' }, true)
				.catch(() => {
					if (token === latest)
						searchError = 'Could not search drivers. Please try again.';
				})
				.finally(() => {
					searches.delete(value);
					if (token === latest) pending = false;
				});
		}, 200);
	}
</script>

<main id="main" class="w-full min-w-0 flex-1">
	<PageIdentity label="Driver directory identity">
		<div
			class="grid min-w-0 items-center gap-4 py-2 md:grid-cols-[minmax(0,1fr)_24rem] md:gap-8"
		>
			<PageHeading title="Drivers"
				>Find drivers and explore their racing records.</PageHeading
			>
			<div class="relative min-w-0">
				<SearchIcon
					class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
					aria-hidden="true"
				/>
				<Input
					class="h-10 pl-10"
					aria-label="Search drivers"
					aria-controls="driver-results"
					placeholder="Search by username or display name..."
					value={search}
					oninput={(e) => searchDrivers(e.currentTarget.value)}
				/>
			</div>
		</div>
	</PageIdentity>
	<div class="mx-auto w-full max-w-screen-2xl p-4 md:p-6">
		<section
			id="driver-results"
			aria-label="Driver matches"
			class="w-full space-y-3"
		>
			{#if data.players?.length}
				<p role="status" class="text-right text-sm text-muted-foreground">
					{#if searching}
						Searching...
					{:else}
						{#if data.players.length >= 25}
							Showing the first 25 matches
						{:else}
							{data.players.length}
							{data.players.length === 1 ? 'match' : 'matches'}
						{/if}
					{/if}
				</p>
			{/if}
			{#if searchError}<p role="alert" class="text-destructive">
					{searchError}
				</p>{/if}
			{#if searching && !data.players?.length}
				<div role="status"><Empty title="Searching for drivers..." /></div>
			{:else if !data.players}
				<Empty title="Search for a driver">
					<p>Type a username or display name to search the directory.</p>
				</Empty>
			{:else if data.players.length}
				<ul aria-busy={searching} class="divide-y border-y text-sm">
					{#each data.players as player (player.id)}
						<li
							class="flex min-w-0 items-center gap-3 pr-4 transition-colors hover:bg-muted/50"
						>
							<a
								class="group flex min-w-0 flex-1 items-center gap-3 px-4 py-3 focus-visible:outline-2 focus-visible:outline-ring focus-visible:outline-offset-[-2px]"
								href="/drivers/{encodeURIComponent(player.lfs_username)}"
							>
								{#if player.flag_code || player.country_code}
									<Flag
										code={player.flag_code}
										fallback={player.country_code}
										class="shrink-0"
									/>
								{/if}
								<span class="min-w-0">
									<span
										class="block break-words font-semibold group-hover:underline"
										>{player.display_name}</span
									>
									<span class="block break-words text-muted-foreground"
										>{player.lfs_username}</span
									>
								</span>
							</a>
							<div class="shrink-0"><CompareButton driver={player} /></div>
						</li>
					{/each}
				</ul>
				<p class="text-xs text-muted-foreground">
					Matches are shown in username order.
				</p>
			{:else}
				<Empty title="No matching drivers">
					<p>No driver matches “{data.search}”.</p>
				</Empty>
			{/if}
		</section>
	</div>
</main>
