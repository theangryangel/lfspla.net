<script lang="ts">
	import { page } from '$app/state';
	import { exploreLinks } from './navigation.js';
	let {
		section,
		onNavigate = () => {},
	}: { section: 'online' | 'explore'; onNavigate?: () => void } = $props();
</script>

<div
	class="mx-auto grid w-full max-w-screen-2xl md:grid-cols-[12rem_minmax(0,1fr)] lg:grid-cols-[15rem_minmax(0,1fr)]"
>
	<div class="border-b border-white/10 p-4 md:border-r md:border-b-0 md:p-5">
		<h2 class="racing-section-title text-white">
			{section === 'online' ? 'Online racing' : 'Explore'}
		</h2>
		<p class="mt-2 text-sm leading-relaxed text-muted-foreground">
			{section === 'online'
				? 'Pick-up racing and organised events across the LFS community.'
				: 'Get to know the drivers, cars and circuits behind the times.'}
		</p>
	</div>
	<div class="min-w-0 p-4 md:p-5">
		{#if section === 'online'}
			<div class="grid gap-4 lg:grid-cols-3">
				<a
					href="/zinc"
					onclick={onNavigate}
					aria-current={page.url.pathname === '/zinc' ? 'page' : undefined}
					class="group relative isolate flex min-h-40 flex-col justify-center overflow-hidden border border-white/15 bg-neutral-900 p-4 transition-colors hover:border-brand-yellow focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-brand-yellow"
				>
					<img
						src="/images/navigation/circuit.webp"
						alt=""
						class="absolute inset-0 -z-20 h-full w-full object-cover object-[center_60%]"
					/>
					<div class="absolute inset-0 -z-10 bg-black/70"></div>
					<div class="max-w-lg">
						<span
							class="inline-flex border border-brand-yellow/40 bg-black/30 px-2 py-0.5 text-xs font-medium text-brand-yellow"
							>Coming soon</span
						>
						<h3 class="racing-section-title mt-2 text-white">
							LFSpla.net ZINC
						</h3>
						<p class="mt-2 text-sm text-white/75">
							Pick-up racing with persistent progression. Earn points, unlock
							cars and build your racing record.
						</p>
						<span
							class="mt-3 inline-block text-sm font-semibold text-brand-yellow underline-offset-4 group-hover:underline"
							>Discover ZINC</span
						>
					</div>
				</a>
				<a
					href="/events"
					onclick={onNavigate}
					aria-current={page.url.pathname === '/events' ? 'page' : undefined}
					class="group relative flex min-h-40 flex-col justify-center border border-white/15 bg-white/5 p-4 transition-colors hover:border-brand-yellow hover:bg-white/10 focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-brand-yellow"
				>
					<h3 class="racing-section-title text-white">Events</h3>
					<p class="mt-2 max-w-lg text-sm text-white/75">
						Find upcoming organised races and league calendars. Plan your next
						race with the LFS community.
					</p>
					<span
						class="mt-3 inline-block text-sm font-semibold text-brand-yellow underline-offset-4 group-hover:underline"
						>Browse events</span
					>
				</a>
				<a
					href="https://www.lfs.net/online/activity"
					onclick={onNavigate}
					class="group relative flex min-h-40 flex-col justify-center border border-white/15 bg-white/5 p-4 transition-colors hover:border-brand-yellow hover:bg-white/10 focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-brand-yellow"
				>
					<h3 class="racing-section-title text-white">Worldwide activity</h3>
					<p class="mt-2 max-w-lg text-sm text-white/75">
						Explore online activity across Live for Speed on the official LFS
						website.
					</p>
					<span
						class="mt-3 inline-block text-sm font-semibold text-brand-yellow underline-offset-4 group-hover:underline"
						>View on LFS.net</span
					>
				</a>
			</div>
		{:else}
			<div class="grid gap-4 lg:grid-cols-2 xl:grid-cols-4">
				{#each exploreLinks as link (link.href)}
					<a
						href={link.href}
						onclick={onNavigate}
						aria-current={page.url.pathname.startsWith(link.href)
							? 'page'
							: undefined}
						class="group relative isolate flex min-h-40 flex-col justify-end overflow-hidden border border-white/15 bg-white/5 p-4 transition-colors hover:border-brand-yellow hover:bg-white/10 focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-brand-yellow"
					>
						{#if link.href === '/drivers'}
							<img
								src="/images/navigation/drivers.webp"
								alt=""
								class="absolute inset-0 -z-20 h-full w-full object-cover object-right opacity-40"
							/>
							<div class="absolute inset-0 -z-10 bg-black/55"></div>
						{:else if link.href === '/tracks'}
							<img
								src="/images/navigation/circuit.webp"
								alt=""
								class="absolute inset-0 -z-20 h-full w-full object-cover opacity-30"
							/>
							<div class="absolute inset-0 -z-10 bg-black/55"></div>
						{:else if link.href === '/vehicles'}
							<img
								src="/images/navigation/formula.png"
								alt=""
								class="absolute -right-4 top-2 -z-10 w-4/5 max-w-64 opacity-25"
							/>
						{/if}
						{#if link.href === '/teams'}
							<span
								class="mb-2 inline-flex w-fit border border-brand-yellow/40 bg-black/30 px-2 py-0.5 text-xs font-medium text-brand-yellow"
								>Coming soon</span
							>
						{/if}
						<h3 class="racing-section-title text-white">{link.label}</h3>
						<p class="mt-2 text-sm text-white/70">{link.description}</p>
					</a>
				{/each}
			</div>
		{/if}
	</div>
</div>
