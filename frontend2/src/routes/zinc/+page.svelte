<script lang="ts">
	import CornerRibbon from '#lib/components/app/CornerRibbon.svelte';
</script>

<main
	id="main"
	class="dark flex w-full min-w-0 flex-1 flex-col bg-[#111113] text-white"
>
	<header class="relative isolate overflow-hidden border-b border-white/15">
		<CornerRibbon />
		<img
			src="/images/navigation/drivers.webp"
			alt=""
			class="absolute inset-0 -z-30 h-full w-full object-cover object-[70%_center] opacity-55"
		/>
		<div
			class="absolute inset-0 -z-20 bg-linear-to-r from-[#111113] via-[#111113]/90 to-[#111113]/35"
		></div>
		<div
			class="absolute inset-x-0 bottom-0 -z-10 h-1/3 bg-linear-to-t from-[#111113] to-transparent"
		></div>
		<div
			class="mx-auto grid w-full max-w-screen-2xl gap-10 px-4 py-12 md:grid-cols-[minmax(0,1fr)_auto] md:items-center md:px-6 md:py-20"
		>
			<div class="max-w-2xl">
				<h1 class="zinc-title text-brand-yellow">ZINC</h1>
				<p class="mt-4 text-xl font-semibold md:text-2xl">Zinc Is Not CTRA.</p>
				<p class="mt-5 max-w-lg text-base leading-relaxed text-white/75">
					Pick-up racing with persistent progression. Earn points, unlock cars
					and build your racing record.
				</p>
				<p class="racing-section-title mt-6 text-brand-yellow">Coming soon</p>
			</div>
			<div
				class="zinc-element hidden w-52 border-2 border-brand-yellow bg-[#111113]/80 p-5 text-brand-yellow md:block"
				aria-hidden="true"
			>
				<span class="font-mono text-lg">30</span>
				<span class="block text-center text-8xl font-bold leading-tight"
					>Zn</span
				>
				<span
					class="mt-2 block text-center text-sm font-semibold tracking-widest"
					>ZINC</span
				>
			</div>
		</div>
	</header>
</main>

<style>
	.zinc-title {
		font-family: 'Barlow Condensed', sans-serif;
		font-size: clamp(6rem, 15vw, 12rem);
		font-weight: 700;
		font-style: italic;
		line-height: 0.85;
		letter-spacing: -0.035em;
	}
	.zinc-element {
		transform: rotate(4deg);
	}
</style>
