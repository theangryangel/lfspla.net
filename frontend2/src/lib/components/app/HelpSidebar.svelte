<script lang="ts">
	import { tick } from 'svelte';
	import { afterNavigate } from '$app/navigation';
	import X from '@lucide/svelte/icons/x';
	import { Button } from '#lib/components/ui/button/index.js';
	import { useHelp, type HelpTopic } from '#lib/help.svelte.js';
	import TeamGuide from './TeamGuide.svelte';
	import HotlapGuide from './HotlapGuide.svelte';

	let { topic, modal }: { topic: HelpTopic | null; modal: boolean } = $props();
	const help = useHelp();
	let panel: HTMLElement | undefined = $state();
	let closeButton: HTMLButtonElement | null = $state(null);

	// On a small screen, following a guide link should reveal its destination.
	afterNavigate(() => {
		if (modal && help.open) void help.close();
	});

	$effect(() => {
		if (help.open && topic) {
			void tick().then(() => closeButton?.focus());
		}
	});
</script>

<svelte:window
	onkeydown={(event) => {
		if (
			help.open &&
			event.key === 'Escape' &&
			!event.defaultPrevented &&
			event.target instanceof Node &&
			panel?.contains(event.target)
		) {
			event.preventDefault();
			help.close();
		}
	}}
/>

{#if help.open && topic}
	<aside
		bind:this={panel}
		role={modal ? 'dialog' : 'complementary'}
		aria-modal={modal ? true : undefined}
		id="page-help"
		aria-labelledby="page-help-title"
		class="fixed inset-y-0 right-0 z-40 flex w-full flex-col border-l bg-background shadow-lg xl:w-88"
	>
		<div
			class="flex shrink-0 items-center justify-between gap-3 border-b px-5 py-4"
		>
			<div class="space-y-1">
				<p class="text-xs font-medium text-muted-foreground">Help</p>
				<h2 id="page-help-title" class="text-base font-semibold">
					{topic === 'teams' ? 'Teams' : 'How to hotlap'}
				</h2>
			</div>
			<Button
				bind:ref={closeButton}
				variant="ghost"
				size="icon"
				aria-label="Close help"
				onclick={help.close}
			>
				<X aria-hidden="true" />
			</Button>
		</div>
		<div
			class="min-h-0 flex-1 overflow-y-auto overscroll-contain p-5 [&_a]:text-primary [&_a]:underline [&_a]:underline-offset-4"
		>
			{#key topic}
				{#if topic === 'teams'}<TeamGuide />{:else}<HotlapGuide />{/if}
			{/key}
		</div>
	</aside>
{/if}
