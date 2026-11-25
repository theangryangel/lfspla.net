<script lang="ts">
	import { page } from '$app/state';
	import type { EraSummary } from '#lib/api.js';
	import { Button } from '#lib/components/ui/button/index.js';
	let {
		eras,
		onNavigate = () => {},
	}: { eras: EraSummary[]; onNavigate?: () => void } = $props();
	const openEras = $derived([...eras].reverse().filter((era) => era.open));
	const historicalEras = $derived([...eras].reverse().filter((era) => !era.open));
</script>

<div
	class="era-menu mx-auto grid w-full max-w-screen-2xl md:grid-cols-[12rem_minmax(0,1fr)] lg:grid-cols-[15rem_minmax(0,1fr)]"
>
	<div class="border-b border-white/10 p-4 md:border-r md:border-b-0 md:p-5">
		<h2 class="racing-section-title text-white">Hotlaps &amp; Drag</h2>
		<p class="mt-2 text-sm leading-relaxed text-muted-foreground">
			Eras group LFS versions with compatible physics and tracks, keeping
			records fair when the game changes.
		</p>
	</div>
	<div
		class={[
			'grid min-w-0 gap-5 p-4 md:p-5',
			openEras.length > 0 &&
				historicalEras.length > 0 &&
				(openEras.length === 1
					? 'lg:grid-cols-[18rem_minmax(0,1fr)]'
					: 'lg:grid-cols-[minmax(0,2fr)_minmax(14rem,1fr)]'),
		]}
	>
		{#if openEras.length}
			<section class="min-w-0">
				<h2 class="text-sm font-semibold text-white">Open eras</h2>
				<div
					class={[
						'mt-3 grid gap-3',
						openEras.length === 1 && 'max-w-72',
						openEras.length > 1 && 'sm:grid-cols-2',
					]}
				>
					{#each openEras as era (era.id)}
						<a
							href={`/hotlaps/${encodeURIComponent(era.id)}`}
							onclick={onNavigate}
							aria-current={page.params.era === era.id ? 'page' : undefined}
							class="group flex min-h-28 min-w-0 flex-col justify-between gap-3 border border-white/15 bg-white/5 p-4 transition-colors hover:border-brand-yellow hover:bg-white/10 focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-brand-yellow aria-[current=page]:border-brand-yellow"
						>
							<div class="min-w-0 space-y-2">
								<h3 class="racing-section-title break-words text-white">
									{era.title}
								</h3>
								<p class="text-sm text-muted-foreground wrap-anywhere">
									Replays recorded with LFS {era.version_requirement}.
								</p>
							</div>
							<span
								class="text-sm font-semibold text-brand-yellow underline-offset-4 group-hover:underline"
								>View charts &amp; rankings</span
							>
						</a>
					{/each}
				</div>
			</section>
		{/if}
		{#if historicalEras.length}
			<section
				class={[
					'min-w-0',
					openEras.length > 0 &&
						'border-t border-white/10 pt-4 lg:-my-5 lg:border-t-0 lg:border-l lg:py-5 lg:pl-5',
				]}
			>
				<h2 class="text-sm font-semibold text-white">Historical eras</h2>
				<div class="mt-2 space-y-1">
					{#each historicalEras as era (era.id)}
						<Button
							variant="ghost"
							href={`/hotlaps/${encodeURIComponent(era.id)}`}
							onclick={onNavigate}
							aria-current={page.params.era === era.id ? 'page' : undefined}
							class="h-auto min-h-8 w-full justify-start whitespace-normal py-1.5 text-left break-words"
							>{era.title}</Button
						>
					{/each}
				</div>
			</section>
		{/if}
		{#if !eras.length}
			<p class="text-sm text-muted-foreground">No hotlap eras available.</p>
		{/if}
	</div>
</div>

<style>
	.era-menu :global([data-slot='button']:hover),
	.era-menu :global([data-slot='button']:focus-visible),
	.era-menu :global([data-slot='button'][aria-current='page']) {
		background-color: var(--color-brand-yellow);
		color: #000;
	}
</style>
