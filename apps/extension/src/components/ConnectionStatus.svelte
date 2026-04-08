<script lang="ts">
	type Props = {
		connected: boolean
		secondsAgo: number
	}

	let { connected, secondsAgo }: Props = $props()
</script>

<div class="connection-status">
	<span class="dot" class:disconnected={!connected}></span>
	{#if connected}
		<span>Live · {secondsAgo}s ago</span>
	{:else}
		<span>Disconnected</span>
	{/if}
</div>
