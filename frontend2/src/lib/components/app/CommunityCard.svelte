<script lang="ts">
	import type { CommunityCard } from '#lib/api.js';

	let { card }: { card: CommunityCard } = $props();
	const content = $derived(card.content);
	const link = $derived(content.link);
	const description = $derived(
		content.type === 'partner'
			? content.description
			: content.type === 'announcement'
				? content.body
				: content.description,
	);
	const fit = $derived(
		content.fit ?? (content.type === 'image' ? 'cover' : 'contain'),
	);
</script>

{#snippet body()}
	<div class="community-card-copy min-w-0 space-y-2 px-5 py-3">
		{#if card.sponsored}
			<p
				class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
			>
				Sponsored
			</p>
		{/if}
		<h3
			id={`community-card-${card.id}-title`}
			class="racing-section-title break-words"
		>
			{card.title}
		</h3>
		{#if description}
			<p
				id={`community-card-${card.id}-description`}
				class="whitespace-pre-line break-words text-sm text-muted-foreground"
			>
				{description}
			</p>
		{/if}
	</div>
	{#if card.image}
		<div class="community-card-artwork">
			<img
				src={card.image.url}
				alt={content.type === 'image' ? content.alt : ''}
				class="size-full"
				class:object-cover={fit === 'cover'}
				class:object-contain={fit === 'contain'}
				loading="lazy"
			/>
		</div>
	{/if}
{/snippet}

{#if link}
	<a
		href={link.url}
		class="community-card community-card-link"
		data-theme={card.theme}
		data-has-image={!!card.image}
		aria-labelledby={`community-card-${card.id}-title`}
		aria-describedby={description
			? `community-card-${card.id}-description`
			: undefined}
		rel={card.sponsored ? 'sponsored' : undefined}
	>
		{@render body()}
	</a>
{:else}
	<article
		class="community-card"
		data-theme={card.theme}
		data-has-image={!!card.image}
	>
		{@render body()}
	</article>
{/if}

<style>
	.community-card {
		--card-accent: var(--foreground);
		display: grid;
		min-width: 0;
		min-height: 7.5rem;
		height: 100%;
		background: var(--card);
		color: var(--foreground);
	}
	.community-card[data-has-image='true'] {
		grid-template-columns: minmax(0, 1fr) minmax(7rem, 36%);
	}
	.community-card[data-theme='cyan'] {
		--card-accent: var(--primary);
	}
	.community-card[data-theme='purple'] {
		--card-accent: var(--time-best);
	}
	.community-card[data-theme='yellow'] {
		--card-accent: var(--brand-yellow);
	}
	.community-card-copy {
		align-self: center;
	}
	.community-card-artwork {
		position: relative;
		min-width: 0;
	}
	.community-card-artwork img {
		position: absolute;
		inset: 0;
	}
	.community-card-link:hover h3 {
		color: var(--card-accent);
	}
	.community-card-link:hover {
		background: var(--muted);
	}
	.community-card-link:focus-visible {
		outline: 2px solid var(--ring);
		outline-offset: -2px;
	}
</style>
