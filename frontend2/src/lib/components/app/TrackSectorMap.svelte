<script lang="ts">
	import type { Attachment } from '#lib/api.js';
	import { delta } from '#lib/format.js';
	import type { SectorComparison } from '#lib/hotlap-comparison.js';

	let {
		outline,
		track,
		comparison,
		hoveredSector = $bindable<number | null>(null),
		focusedSector = $bindable<number | null>(null),
		selectedSector = $bindable<number | null>(null),
	}: {
		outline: Attachment;
		track: string;
		comparison: SectorComparison[] | null;
		hoveredSector?: number | null;
		focusedSector?: number | null;
		selectedSector?: number | null;
	} = $props();

	const componentId = $props.id();
	const filterId = `${componentId}-sector-outline`;
	const activeSector = $derived(
		hoveredSector ?? focusedSector ?? selectedSector,
	);
	const fragment = $derived(
		comparison ? `track-sectors-${comparison.length}` : 'track',
	);

	function select(index: number) {
		selectedSector = selectedSector === index ? null : index;
	}
</script>

<div class="min-w-0 bg-card/60 p-4">
	<div
		class="relative h-64 w-full"
		style:--sector-1="var(--muted-foreground)"
		style:--sector-2="var(--muted-foreground)"
		style:--sector-3="var(--muted-foreground)"
		style:--sector-4="var(--muted-foreground)"
	>
		<!-- Existing files and incompatible sector counts retain a neutral outline. -->
		<img
			src={outline.url}
			alt=""
			class="pointer-events-none absolute inset-0 size-full object-contain"
		/>
		<svg
			class="absolute inset-0 size-full"
			viewBox="0 0 160 100"
			role={comparison ? 'group' : 'img'}
			aria-describedby="hotlap-sector-legend"
			aria-label={`${track} split map${comparison ? ', compared with the current world record' : ''}`}
		>
			<defs>
				<filter
					id={filterId}
					filterUnits="userSpaceOnUse"
					x="-4"
					y="-4"
					width="168"
					height="108"
					color-interpolation-filters="sRGB"
				>
					<feMorphology
						in="SourceAlpha"
						operator="dilate"
						radius="1.2"
						result="outer"
					/>
					<feMorphology
						in="SourceAlpha"
						operator="dilate"
						radius="0.45"
						result="inner"
					/>
					<feComposite in="outer" in2="inner" operator="out" result="ring" />
					<feFlood
						flood-color="var(--ring)"
						flood-opacity="0.8"
						result="colour"
					/>
					<feComposite in="colour" in2="ring" operator="in" />
				</filter>
			</defs>
			{#each comparison ?? [] as sector, index (index)}
				<g
					role="button"
					tabindex="0"
					aria-label={`Split ${index + 1}, ${delta(sector.delta)} against the world record`}
					aria-pressed={selectedSector === index}
					class="cursor-pointer outline-none"
					onpointerenter={(event) => {
						if (event.pointerType !== 'touch') hoveredSector = index;
					}}
					onpointerleave={() => {
						if (hoveredSector === index) hoveredSector = null;
					}}
					onfocus={() => (focusedSector = index)}
					onblur={() => {
						if (focusedSector === index) focusedSector = null;
					}}
					onclick={() => select(index)}
					onkeydown={(event) => {
						if (event.key === 'Enter' || event.key === ' ') {
							event.preventDefault();
							select(index);
						}
						if (event.key === 'Escape') selectedSector = null;
					}}
				>
					<!-- Transparent copies provide per-sector hover targets without changing the attachment. -->
					<use
						href={`${outline.url}#sector-${index + 1}`}
						opacity="0"
						pointer-events="stroke"
					/>
					<use
						href={`${outline.url}#sector-${index + 1}`}
						filter={`url(#${filterId})`}
						pointer-events="none"
						class="transition-opacity duration-150 motion-reduce:transition-none"
						opacity={activeSector === index ? 1 : 0}
					/>
				</g>
			{/each}
			<!-- Keep the original sector colours and timing markers above the outline. -->
			<use
				href={`${outline.url}#${fragment}`}
				width="160"
				height="100"
				pointer-events="none"
			/>
		</svg>
	</div>
</div>
