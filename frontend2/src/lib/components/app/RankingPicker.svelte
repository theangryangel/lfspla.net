<script lang="ts">
	import { goto } from '$app/navigation';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';
	import RankingList from '$lib/components/app/RankingList.svelte';
	import { useApi, type EraSummary } from '$lib/api.js';
	import { useSession } from '$lib/session.svelte.js';

	let { era, selected }: { era: EraSummary; selected?: string } = $props();
	const api = useApi();
	const session = useSession();
	let visible = $state(false);
	let search = $state('');
	let participation = $state<'all' | 'participating' | 'missed'>('all');
	let progress = $state<Record<
		string,
		{ completed: number; total: number }
	> | null>(null);
	let loading = $state(false);
	let failed = $state(false);
	let retry = $state(0);
	let opener: HTMLElement | null = null;
	let dialogContent = $state<HTMLElement | null>(null);

	export function open(from?: HTMLElement | null, query = '') {
		opener = from ?? null;
		search = query;
		participation = 'all';
		visible = true;
	}

	// Personal progress is only requested while the picker is needed. Ignore
	// late responses after closing, changing era, or signing out.
	$effect(() => {
		const eraId = era.id;
		const signedIn = session.signedIn;
		void retry;
		progress = null;
		failed = false;
		loading = false;
		if (!visible || !signedIn || !era.rankings.length) return;
		let cancelled = false;
		loading = true;
		api.ranking
			.listEraRankings({ era: eraId })
			.then((response) => {
				if (cancelled) return;
				progress = Object.fromEntries(
					response.items.map((ranking) => [
						ranking.id,
						{
							completed: ranking.my_progress?.completed_charts ?? 0,
							total: ranking.my_progress?.total_charts ?? 0,
						},
					]),
				);
			})
			.catch(() => {
				if (!cancelled) failed = true;
			})
			.finally(() => {
				if (!cancelled) loading = false;
			});
		return () => {
			cancelled = true;
		};
	});

	const matches = $derived(
		era.rankings.filter((ranking) => {
			const own = progress?.[ranking.id];
			const participating =
				!session.signedIn ||
				participation === 'all' ||
				(participation === 'participating' && (own?.completed ?? 0) > 0) ||
				(participation === 'missed' &&
					own !== undefined &&
					own.completed < own.total);
			return (
				participating &&
				`${ranking.title} ${ranking.description}`
					.toLowerCase()
					.includes(search.trim().toLowerCase())
			);
		}),
	);

	function choose(event: MouseEvent) {
		// Keep ordinary link behaviour for opening standings in another tab.
		if (
			event.button !== 0 ||
			event.metaKey ||
			event.ctrlKey ||
			event.shiftKey ||
			event.altKey
		)
			return;
		event.preventDefault();
		const href = (event.currentTarget as HTMLAnchorElement).href;
		visible = false;
		void goto(href);
	}
</script>

<Dialog.Root bind:open={visible}>
	<Dialog.Content
		bind:ref={dialogContent}
		tabindex={-1}
		class="max-h-[calc(100dvh-2rem)] gap-3 overflow-y-auto rounded-sm sm:max-w-2xl"
		onOpenAutoFocus={(event) => {
			event.preventDefault();
			dialogContent?.focus({ preventScroll: true });
		}}
		onCloseAutoFocus={(event) => {
			if (opener?.isConnected) {
				event.preventDefault();
				opener.focus({ preventScroll: true });
			}
			opener = null;
		}}
	>
		<Dialog.Header class="pr-8">
			<Dialog.Title class="racing-section-title">Choose a ranking</Dialog.Title>
			<Dialog.Description
				>{era.title} · Rankings available in this era.</Dialog.Description
			>
		</Dialog.Header>
		<Input
			aria-label="Search rankings"
			placeholder="Search rankings…"
			bind:value={search}
		/>
		<div class="flex flex-wrap items-center justify-between gap-2">
			<ToggleGroup.Root
				type="single"
				variant="outline"
				aria-label="Ranking participation"
				bind:value={participation}
			>
				<ToggleGroup.Item value="all">All ranks</ToggleGroup.Item>
				<ToggleGroup.Item
					value="participating"
					disabled={!session.signedIn || !progress}
					>Participating</ToggleGroup.Item
				>
				<ToggleGroup.Item
					value="missed"
					disabled={!session.signedIn || !progress}>Missed</ToggleGroup.Item
				>
			</ToggleGroup.Root>
			<span class="text-muted-foreground"
				>{matches.length} of {era.rankings.length}</span
			>
		</div>
		{#if !session.signedIn}
			<Button
				variant="outline"
				size="sm"
				class="justify-self-start"
				onclick={session.signIn}>Sign in to see your progress</Button
			>
		{:else if loading}
			<p class="text-muted-foreground" role="status">Loading your progress…</p>
		{:else if failed}
			<div class="flex items-center gap-2" role="status">
				<span class="text-muted-foreground"
					>Your progress could not be loaded.</span
				>
				<Button variant="outline" size="sm" onclick={() => retry++}
					>Retry</Button
				>
			</div>
		{/if}
		<RankingList
			era={era.id}
			rankings={matches}
			{selected}
			progress={session.signedIn ? progress : null}
			onChoose={choose}
			empty={era.rankings.length
				? 'No matching rankings. Try another search or participation filter.'
				: 'No rankings in this era yet.'}
		/>
	</Dialog.Content>
</Dialog.Root>
