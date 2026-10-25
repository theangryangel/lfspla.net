<script lang="ts">
	import { navigating } from '$app/state';
	import { onMount } from 'svelte';
	import { BProgress } from '@bprogress/core';
	import { beginLoading, loading } from '#lib/loading-progress.js';

	let visible = $state(false);

	$effect(() => {
		if (!navigating.to || navigating.willUnload) return;
		return beginLoading();
	});

	onMount(() => {
		const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
		const configure = () =>
			BProgress.configure({
				showSpinner: false,
				trickle: !reducedMotion.matches,
				speed: reducedMotion.matches ? 0 : 200,
			});
		configure();
		reducedMotion.addEventListener('change', configure);
		let timer: ReturnType<typeof setTimeout> | undefined;
		const unsubscribe = loading.subscribe((pending) => {
			clearTimeout(timer);
			if (pending) {
				timer = setTimeout(() => {
					visible = true;
					BProgress.start();
				}, 150);
			} else {
				if (visible) BProgress.done();
				visible = false;
			}
		});
		return () => {
			unsubscribe();
			clearTimeout(timer);
			reducedMotion.removeEventListener('change', configure);
			BProgress.remove();
		};
	});
</script>

<div role="status" class="sr-only">{visible ? 'Loading…' : ''}</div>

<style>
	:global(.bprogress) {
		--bprogress-color: var(--primary);
		--bprogress-height: 2px;
		--bprogress-box-shadow: none;
	}
</style>
