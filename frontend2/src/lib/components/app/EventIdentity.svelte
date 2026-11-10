<script lang="ts">
	import type { EventDetail, Standing } from '#lib/api.js';
	import { lapTime, delta } from '#lib/format.js';
	import PageIdentity from './PageIdentity.svelte';
	import EventRules from './EventRules.svelte';
	import PageHeading from './PageHeading.svelte';
	import RecordHighlight from './RecordHighlight.svelte';
	import Flag from './Flag.svelte';
	import { Button } from '#lib/components/ui/button/index.js';

	let {
		event,
		status,
		leader,
	}: {
		event: EventDetail;
		status: 'Upcoming' | 'Live' | 'Completed';
		leader?: Standing;
	} = $props();
	const mode = $derived(event.game?.source.type);
	const identity = $derived(
		mode === 'metronome'
			? {
					description: `Match the target lap time${event.game?.source.type === 'metronome' ? ` of ${lapTime(event.game.source.target_ms)}` : ''}. The smallest absolute difference wins.`,
					variant: 'cyan' as const,
				}
			: mode === 'shortcut'
				? {
						description:
							'Find the fastest route through the layout. Your fastest completed attempt counts.',
						variant: 'purple' as const,
					}
				: {
						description:
							'Reach checkpoints to keep the timer alive. Most checkpoints wins, then longest survival time.',
						variant: 'yellow' as const,
					},
	);
</script>

<div class="event-identity" data-mode={mode}>
	<PageIdentity label="Event identity">
		<div
			class="relative isolate grid min-w-0 items-center gap-6 overflow-hidden py-2"
			class:xl:grid-cols-2={mode === 'bomb'}
			class:xl:gap-8={mode === 'bomb'}
		>
			{#if mode}
				<svg
					class="mode-art pointer-events-none absolute inset-y-0 right-0 -z-10 h-full w-auto max-w-full"
					viewBox="0 0 280 140"
					preserveAspectRatio="xMaxYMid meet"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					aria-hidden="true"
				>
					{#if mode === 'metronome'}
						<path
							d="M0 100H280M20 85V115M50 75V125M80 85V115M110 65V135M140 85V115M170 75V125M200 85V115M230 65V135M260 85V115"
						/>
						<path d="M110 18V60M100 30L110 18L120 30" stroke-width="4" />
					{:else if mode === 'shortcut'}
						<path
							d="M0 120H90L135 30H220L260 85H280"
							stroke-width="16"
							opacity=".4"
						/>
						<path d="M0 120H90L135 30H220L260 85H280" stroke-dasharray="6 8" />
						<path d="M85 120L220 30M198 32L220 30L211 50" stroke-width="3" />
					{:else}
						<circle cx="185" cy="78" r="52" stroke-width="12" opacity=".3" />
						<path d="M185 26A52 52 0 1 1 134 88" stroke-width="12" />
						<path d="M185 43V78L208 91M171 8H199M185 8V20" stroke-width="4" />
					{/if}
				</svg>
			{/if}
			<div class="min-w-0 py-2">
				<PageHeading title={event.title}
					>{#if mode}<p>{identity.description}</p>{/if}</PageHeading
				>
				{#if event.join_url && status === 'Live'}
					<div class="mt-5 flex flex-wrap gap-2">
						<Button href={event.join_url}>Join server</Button>
					</div>
				{/if}
			</div>
			{#if event.game?.source.type === 'bomb'}<EventRules
					game={event.game}
				/>{/if}
		</div>
		{#snippet highlight()}
			{#if mode}
				<RecordHighlight
					title={mode === 'metronome'
						? 'Target time'
						: leader
							? status === 'Completed'
								? 'Winning run'
								: 'Leading run'
							: mode === 'bomb'
								? 'Checkpoint timer'
								: 'The challenge'}
					variant={identity.variant}
				>
					<p
						class="font-mono text-4xl font-semibold tracking-tight tabular-nums"
					>
						{#if event.game?.source.type === 'metronome'}{lapTime(
								event.game.source.target_ms,
							)}
						{:else if mode === 'bomb' && leader}{leader.checkpoint_count}<span
								class="ml-2 font-sans text-sm font-medium tracking-normal"
								>checkpoints</span
							>
						{:else if event.game?.source.type === 'bomb'}{event.game.source
								.checkpoint_timeout_secs}<span
								class="ml-2 font-sans text-sm font-medium tracking-normal"
								>seconds</span
							>
						{:else if leader}{lapTime(leader.time_ms)}
						{:else}<span class="font-sans text-2xl tracking-normal"
								>Every millisecond counts.</span
							>{/if}
					</p>
					{#if leader}
						<div class="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm">
							<Flag code={leader.flag_code} fallback={leader.country_code} />
							<a
								class="font-medium hover:underline"
								href={`/drivers/${encodeURIComponent(leader.username)}`}
								>{leader.display_name}</a
							>
						</div>
						{#if mode === 'metronome'}<p class="text-sm">
								{status === 'Completed'
									? 'Winning difference'
									: 'Closest so far'}:
								<span class="font-mono tabular-nums"
									>{delta(leader.delta_ms)}</span
								>
							</p>
						{:else if mode === 'bomb'}<p class="text-sm">
								Survived <span class="font-mono tabular-nums"
									>{lapTime(leader.survival_ms)}</span
								>
							</p>{/if}
					{:else}<p class="text-sm">
							{status === 'Upcoming'
								? 'Get ready to set the first mark.'
								: status === 'Completed'
									? 'No runs were recorded.'
									: 'The first mark is yours to set.'}
						</p>{/if}
				</RecordHighlight>
			{/if}
		{/snippet}
	</PageIdentity>
</div>

<style>
	.event-identity {
		--mode-accent: var(--primary);
	}
	.event-identity[data-mode='shortcut'] {
		--mode-accent: light-dark(oklch(0.48 0.22 293), oklch(0.77 0.15 293));
	}
	.event-identity[data-mode='bomb'] {
		--mode-accent: light-dark(oklch(0.51 0.13 70), var(--brand-yellow));
	}
	.mode-art {
		color: var(--mode-accent);
		opacity: 0.12;
	}
</style>
