<script lang="ts">
	import { cn } from '$lib/utils.js';

	/**
	 * A country's flag, from flagcdn.
	 *
	 * Nothing is drawn for a driver without a nation, so callers can hand over an
	 * optional code without guarding it themselves. The code stays in the `title`
	 * so the nation is still readable where the flag is the only marker.
	 */
	let { code, class: className }: { code?: string | null; class?: string } =
		$props();

	const slug = $derived(code?.toLowerCase() ?? '');
</script>

{#if code}
	<img
		src="https://flagcdn.com/20x15/{slug}.png"
		srcset="https://flagcdn.com/40x30/{slug}.png 2x"
		width="20"
		height="15"
		loading="lazy"
		alt="{code} flag"
		title={code}
		class={cn(
			'inline-block shrink-0 rounded-[1px] align-middle shadow-sm',
			className,
		)}
	/>
{/if}
