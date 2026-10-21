<script lang="ts">
	import { onMount, untrack, type Snippet } from 'svelte';
	import emblaCarouselSvelte from 'embla-carousel-svelte';
	import Fade from 'embla-carousel-fade';
	import type { EmblaCarouselType } from 'embla-carousel';
	import ChevronLeftIcon from '@lucide/svelte/icons/chevron-left';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import PauseIcon from '@lucide/svelte/icons/pause';
	import PlayIcon from '@lucide/svelte/icons/play';
	import { Button } from '#lib/components/ui/button/index.js';
	import Panel from './Panel.svelte';

	let {
		id,
		label,
		title,
		variant,
		slides,
		slideContent,
		autoplayStep = 0,
	}: {
		id: string;
		label: string;
		title?: string;
		variant: 'purple' | 'cyan';
		slides: { id?: string; title: string; content?: Snippet }[];
		slideContent?: Snippet<[number]>;
		autoplayStep?: number;
	} = $props();

	let api = $state<EmblaCarouselType>();
	let selected = $state(0);
	let canScrollPrev = $state(false);
	let canScrollNext = $state(false);
	let paused = $state(false);
	let hovering = $state(false);
	let focused = $state(false);
	let reducedMotion = $state(true);
	const plugins = [Fade()];
	onMount(() => {
		const preference = window.matchMedia('(prefers-reduced-motion: reduce)');
		const syncMotion = () => (reducedMotion = preference.matches);
		syncMotion();
		preference.addEventListener('change', syncMotion);
		return () => preference.removeEventListener('change', syncMotion);
	});

	// The homepage alternates ticks between panes so their fades never coincide.
	$effect(() => {
		const step = autoplayStep;
		if (!step) return;
		untrack(() => {
			if (
				!api ||
				slides.length < 2 ||
				paused ||
				hovering ||
				focused ||
				document.hidden ||
				jump()
			)
				return;
			api.scrollNext();
		});
	});

	function sync() {
		if (!api) return;
		selected = api.selectedScrollSnap();
		canScrollPrev = api.canScrollPrev();
		canScrollNext = api.canScrollNext();
	}

	function init(event: CustomEvent<EmblaCarouselType>) {
		api = event.detail;
		api.on('select', sync).on('reInit', sync);
		sync();
	}

	function jump() {
		return reducedMotion;
	}
</script>

<section
	class="flex min-w-0 flex-col"
	aria-roledescription="carousel"
	aria-label={label}
	onpointerenter={() => (hovering = true)}
	onpointerleave={() => (hovering = false)}
	onfocusin={() => (focused = true)}
	onfocusout={(event) =>
		(focused = event.currentTarget.contains(
			event.relatedTarget as Node | null,
		))}
>
	<Panel
		title={title ?? slides[selected]?.title ?? label}
		titleId={`${id}-title`}
		appearance="data"
		surface={false}
		flush
		stretch
	>
		{#snippet action()}
			{#if slides.length > 1}
				<div class="flex items-center gap-1 text-muted-foreground">
					<Button
						variant="ghost"
						size="icon"
						aria-label={`${paused ? 'Resume' : 'Pause'} autoplay in ${label}`}
						aria-pressed={paused}
						onclick={() => (paused = !paused)}
					>
						{#if paused}<PlayIcon />{:else}<PauseIcon />{/if}
					</Button>
					<Button
						variant="ghost"
						size="icon"
						aria-label={`Previous slide in ${label}`}
						aria-controls={`${id}-slides`}
						disabled={!canScrollPrev}
						onclick={() => api?.scrollPrev(jump())}
					>
						<ChevronLeftIcon data-icon="inline-start" />
					</Button>
					<Button
						variant="ghost"
						size="icon"
						aria-label={`Next slide in ${label}`}
						aria-controls={`${id}-slides`}
						disabled={!canScrollNext}
						onclick={() => api?.scrollNext(jump())}
					>
						<ChevronRightIcon data-icon="inline-end" />
					</Button>
				</div>
			{/if}
		{/snippet}
		<div
			class="min-h-0 flex-1 overflow-hidden"
			use:emblaCarouselSvelte={{ options: { loop: true }, plugins }}
			onemblaInit={init}
		>
			<div id={`${id}-slides`} class="flex h-full touch-pan-y touch-pinch-zoom">
				{#each slides as slide, index (slide.id ?? slide.title)}
					<div
						class="grid min-w-0 flex-[0_0_100%]"
						role="group"
						aria-roledescription="slide"
						aria-label={`${index + 1} of ${slides.length}: ${slide.title}`}
						inert={index !== selected}
					>
						{#if slideContent}
							{@render slideContent(index)}
						{:else if slide.content}
							{@render slide.content()}
						{/if}
					</div>
				{/each}
			</div>
		</div>
		<div
			class="flex h-8 justify-center"
			aria-label={slides.length > 1 ? `${label} slides` : undefined}
		>
			{#if slides.length > 1}
				{#each slides as slide, index (slide.id ?? slide.title)}
					<button
						type="button"
						class="carousel-dot flex h-8 w-4 cursor-pointer items-center justify-center rounded-sm focus-visible:outline-2 focus-visible:outline-ring"
						data-active={index === selected}
						data-variant={variant}
						aria-label={`Show ${slide.title}`}
						aria-current={index === selected ? 'true' : undefined}
						aria-controls={`${id}-slides`}
						onclick={() => api?.scrollTo(index, jump())}
					>
						<span class="size-2 rounded-full bg-border"></span>
					</button>
				{/each}
			{/if}
		</div>
		<p
			class="sr-only"
			aria-live={paused || focused || hovering ? 'polite' : 'off'}
			aria-atomic="true"
		>
			{slides[selected]?.title}, slide {selected + 1} of {slides.length}
		</p>
	</Panel>
</section>

<style>
	.carousel-dot[data-active='true'][data-variant='purple'] span {
		background: var(--time-best);
	}
	.carousel-dot[data-active='true'][data-variant='cyan'] span {
		background: var(--primary);
	}
</style>
