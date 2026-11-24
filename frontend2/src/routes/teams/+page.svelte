<script lang="ts">
	import Empty from '#lib/components/app/Empty.svelte';
	import PageIdentity from '#lib/components/app/PageIdentity.svelte';
	import PageHeading from '#lib/components/app/PageHeading.svelte';
</script>

<main id="main" class="w-full min-w-0 flex-1">
	<PageIdentity label="Team directory identity">
		<div class="py-2">
			<PageHeading title="Teams">Teams and the drivers behind them.</PageHeading
			>
		</div>
	</PageIdentity>
	<div class="mx-auto w-full max-w-screen-2xl space-y-4 p-4 md:p-6">
		<Empty title="Coming soon">
			<p>The team directory is on its way.</p>
		</Empty>
	</div>
</main>
