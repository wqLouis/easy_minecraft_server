<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { ArrowLeftIcon, RefreshCwIcon, SendIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { isAuthenticated, isConfigured, getApi } from "$lib/api";

  let logs = $state<string[]>([]);
  let loading = $state(true);
  let cmd = $state("");
  let sending = $state(false);
  const id = $derived($page.params.id);

  onMount(() => {
    if (!isConfigured() || !isAuthenticated()) { goto("/"); return; }
    fetchLogs();
  });

  async function fetchLogs() {
    loading = true;
    try { logs = (await getApi().get<{ logs: string[] }>(`/api/instances/${id}/logs?tail=200`)).logs; }
    catch { goto("/"); }
    finally { loading = false }
  }

  async function send() {
    const c = cmd.trim(); if (!c || sending) return;
    sending = true; cmd = "";
    try { await getApi().post(`/api/instances/${id}/command`, { command: c }); toast.success("Command sent"); }
    catch (e) { toast.error("Failed", { description: e instanceof Error ? e.message : "" }); }
    sending = false;
  }
</script>

<div class="mx-auto max-w-4xl px-6 py-6">
  <button onclick={() => goto(`/servers/${id}`)} class="mb-4 text-sm text-muted-foreground hover:text-foreground"><ArrowLeftIcon class="inline size-4" /> Back</button>
  <div class="mb-4 flex items-center justify-between">
    <h1 class="text-xl font-semibold">Server Logs</h1>
    <Button variant="outline" size="sm" onclick={fetchLogs} disabled={loading}><RefreshCwIcon class={loading ? "size-4 animate-spin" : "size-4"} /></Button>
  </div>
  {#if loading}
    <div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
  {:else}
    <div class="mb-3 max-h-[55dvh] min-h-64 overflow-auto rounded-lg border bg-card p-4 font-mono text-xs leading-relaxed">
      {#if logs.length === 0}
        <p class="py-12 text-center text-muted-foreground italic">No logs yet.</p>
      {:else}
        {#each logs as line, i}<div class="whitespace-pre-wrap break-all px-1 py-0.5 {i % 2 === 0 ? '' : 'bg-muted/10'}">{line}</div>{/each}
      {/if}
    </div>
    <div class="flex gap-2">
      <Input bind:value={cmd} onkeydown={(e) => e.key === "Enter" && send()} placeholder="Type a command…" disabled={sending} class="font-mono" />
      <Button onclick={send} disabled={!cmd.trim() || sending}>{#if sending}<RefreshCwIcon class="size-4 animate-spin" />{:else}<SendIcon class="size-4" />{/if} Send</Button>
    </div>
  {/if}
</div>
