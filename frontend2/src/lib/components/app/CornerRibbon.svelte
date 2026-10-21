<script lang="ts">
	import { cn } from '#lib/utils.js';

	/** Overlays the corner of its nearest positioned container. */
	let {
		label = 'WIP',
		corner = 'right',
		class: className,
	}: {
		label?: string;
		corner?: 'left' | 'right';
		class?: string;
	} = $props();
</script>

<div
	class={cn(
		'corner-ribbon pointer-events-none absolute top-0 z-10 aspect-square h-full max-h-36',
		corner === 'right' ? 'right-0' : 'left-0',
		className,
	)}
	class:left={corner === 'left'}
>
	<span
		class="absolute text-center text-2xl leading-8 font-bold tracking-wider text-white uppercase"
		>{label}</span
	>
</div>

<style>
	.corner-ribbon::before {
		content: '';
		position: absolute;
		inset: 0;
		background: var(--color-red-600);
		clip-path: polygon(0 0, 50% 0, 100% 50%, 100% 100%);
	}
	.corner-ribbon span {
		left: 62.5%;
		top: 37.5%;
		transform: translate(-50%, -50%) rotate(45deg);
	}
	.corner-ribbon.left::before {
		clip-path: polygon(50% 0, 100% 0, 0 100%, 0 50%);
	}
	.corner-ribbon.left span {
		left: 37.5%;
		transform: translate(-50%, -50%) rotate(-45deg);
	}
</style>
