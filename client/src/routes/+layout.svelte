<script lang="ts">
	import './layout.css';
	import favicon from '$lib/assets/favicon.svg';
  import Nav from '$components/nav.svelte';
  import Footer from '$components/footer.svelte';
	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<main class="flex flex-col min-h-screen">
  <Nav />
	{@render children()}
	<Footer />
</main>


