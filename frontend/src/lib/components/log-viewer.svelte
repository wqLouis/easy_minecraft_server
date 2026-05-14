<script lang="ts">
	import * as Dialog from "$lib/components/ui/dialog/index.js";
	import { Button } from "$lib/components/ui/button/index.js";
	import { RefreshCwIcon } from "@lucide/svelte";

	let {
		open = $bindable(false),
		serverName,
		logs,
		loading,
		onRefresh,
	}: {
		open: boolean;
		serverName: string;
		logs: string[];
		loading: boolean;
		onRefresh: () => void;
	} = $props();

	let logContainer = $state<HTMLDivElement | null>(null);

	$effect(() => {
		if (open && logContainer) {
			requestAnimationFrame(() => {
				logContainer!.scrollTop = logContainer!.scrollHeight;
			});
		}
	});
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="max-w-2xl sm:max-w-3xl">
		<Dialog.Header>
			<Dialog.Title>Server Logs — {serverName}</Dialog.Title>
			<Dialog.Description>
				Server console output.
			</Dialog.Description>
		</Dialog.Header>

		<div class="flex items-center justify-between">
			<Button variant="outline" size="sm" onclick={onRefresh} disabled={loading}>
				{#if loading}
					<RefreshCwIcon class="size-3 animate-spin" />
				{:else}
					<RefreshCwIcon class="size-3" />
				{/if}
				Refresh
			</Button>
		</div>

		<div
			bind:this={logContainer}
			class="bg-muted/50 max-h-96 min-h-48 overflow-y-auto rounded-lg border p-4 font-mono text-xs leading-relaxed"
		>
			{#if logs.length === 0}
				<p class="text-muted-foreground italic">No log entries yet.</p>
			{:else}
				{#each logs as line}
					<div class="whitespace-pre-wrap break-all">{line}</div>
				{/each}
			{/if}
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (open = false)}>Close</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
