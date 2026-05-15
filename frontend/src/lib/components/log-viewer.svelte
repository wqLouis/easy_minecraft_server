<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { RefreshCwIcon } from "@lucide/svelte";
  import { toast } from "svelte-sonner";
  import { getApi, isConfigured, isAuthenticated } from "$lib/api";

  let {
    open = $bindable(false),
    serverName = "",
    serverId = "",
  }: { open: boolean; serverName: string; serverId: string } = $props();

  let logs = $state<string[]>([]);
  let loading = $state(false);
  let container = $state<HTMLDivElement | null>(null);

  $effect(() => {
    if (open && serverId) fetchLogs();
  });

  $effect(() => {
    if (open && container) requestAnimationFrame(() => container!.scrollTop = container!.scrollHeight);
  });

  async function fetchLogs() {
    if (!isConfigured() || !isAuthenticated()) return;
    loading = true;
    try {
      const res = await getApi().get<{ logs: string[] }>(`/api/instances/${serverId}/logs?tail=100`);
      logs = res.logs;
    } catch (e: unknown) {
      toast.error("Failed to load logs", { description: e instanceof Error ? e.message : "" });
    } finally { loading = false }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="max-w-2xl sm:max-w-3xl">
    <Dialog.Header>
      <Dialog.Title>Server Logs — {serverName}</Dialog.Title>
      <Dialog.Description>Server console output.</Dialog.Description>
    </Dialog.Header>

    <div class="mb-3 flex items-center gap-2">
      <Button variant="outline" size="sm" onclick={fetchLogs} disabled={loading}>
        <RefreshCwIcon class={loading ? "size-3 animate-spin" : "size-3"} /> Refresh
      </Button>
    </div>

    <div bind:this={container} class="bg-muted/50 max-h-96 min-h-48 overflow-y-auto rounded-lg border p-4 font-mono text-xs leading-relaxed">
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
