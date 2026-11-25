<script lang="ts">
	import { page } from '$app/state';

	const era = $derived(page.data.era);
	const base = $derived(era ? `/hotlaps/${encodeURIComponent(era.id)}` : '/');
</script>

<div class="space-y-6 text-sm">
	<p class="leading-relaxed text-muted-foreground">
		Record a clean run in Live for Speed, save the replay, then upload it here.
		Keep this guide open while you work through the steps.
	</p>
	<ol
		class="list-decimal space-y-5 pl-5 marker:font-semibold marker:text-foreground"
	>
		<li class="space-y-2 pl-1">
			<h3 class="font-semibold">Choose a chart and LFS version</h3>
			<p class="leading-relaxed text-muted-foreground">
				Browse the <a href={`${base}/charts`}>charts</a> for a track and vehicle
				combination you want to drive. Your replay must match the era's accepted
				LFS versions{#if era}: <strong class="font-medium text-foreground"
						>{era.version_requirement}</strong
					>{/if}.
			</p>
			{#if era && !era.open}
				<p class="leading-relaxed text-muted-foreground">
					This era is closed to uploads. Choose an open era from the Hotlaps
					menu to submit a new time.
				</p>
			{/if}
		</li>
		<li class="space-y-2 pl-1">
			<h3 class="font-semibold">Record your run</h3>
			<p class="leading-relaxed text-muted-foreground">
				In LFS, choose Hotlapping and select your track and vehicle. Drive a
				clean run that passes LFS's hotlap validity check (HLVC), then save the
				single player replay (.spr). You'll find saved replays in your LFS
				installation's
				<code class="text-foreground">data/spr</code> folder.
			</p>
		</li>
		<li class="space-y-2 pl-1">
			<h3 class="font-semibold">Sign in and upload</h3>
			<p class="leading-relaxed text-muted-foreground">
				Sign in with the LFS account used to record your replay. Choose
				<strong class="font-medium text-foreground"
					>Upload hotlap or drag</strong
				>
				in the page toolbar, then select or drop your .spr files. You can upload more
				than one replay at a time.
			</p>
		</li>
		<li class="space-y-2 pl-1">
			<h3 class="font-semibold">Check the result</h3>
			<p class="leading-relaxed text-muted-foreground">
				Uploading is followed by validation. Visit
				<a href={`${base}/submissions`}>My submissions</a> to check progress or read
				why a replay wasn't accepted. Validated personal bests appear on the matching
				chart and contribute to eligible rankings.
			</p>
		</li>
	</ol>
	<section class="space-y-2 border-t pt-5">
		<h3 class="font-semibold">Working towards a rank?</h3>
		<p class="leading-relaxed text-muted-foreground">
			Open a <a href={`${base}/rankings`}>ranking</a> and choose
			<strong class="font-medium text-foreground"
				>Rank info &amp; progress</strong
			>
			for its scoring rules, combinations and your progress.
		</p>
	</section>
</div>
