<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { ArrowLeftIcon, SendIcon, TerminalIcon, RefreshCwIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { isAuthenticated, isConfigured, getApi } from "$lib/api";

  let running = $state(false);
  let loading = $state(true);
  let cmd = $state("");
  let sending = $state(false);
  let output = $state<{ text: string; cls: string }[]>([]);
  const id = $derived($page.params.id);

  onMount(async () => {
    if (!isConfigured() || !isAuthenticated()) { goto("/"); return; }
    try { running = (await getApi().get<{ status: Record<string, unknown> }>(`/api/instances/${id}`)).status.running === true; }
    catch { goto("/"); } finally { loading = false; }
  });

  async function send() {
    const c = cmd.trim(); if (!c || !running || sending) return;
    sending = true;
    output = [...output, { text: `> ${c}`, cls: "text-green-600 font-semibold" }]; cmd = "";
    try {
      await getApi().post(`/api/instances/${id}/command`, { command: c });
      output = [...output, { text: "Sent.", cls: "text-muted-foreground" }];
    } catch (e) { output = [...output, { text: `Error: ${e instanceof Error ? e.message : ""}`, cls: "text-red-500" }]; }
    sending = false;
  }
</script>

<div class="mx-auto max-w-4xl px-6 py-6">
  <button onclick={() => goto(`/servers/${id}`)} class="mb-4 text-sm text-muted-foreground hover:text-foreground"><ArrowLeftIcon class="inline size-4" /> Back</button>
  <div class="mb-4 flex items-center gap-2">
    <TerminalIcon class="size-5" />
    <h1 class="flex-1 text-xl font-semibold">Console <span class="text-sm font-normal text-muted-foreground">({running ? "Running" : "Stopped"})</span></h1>
  </div>
  {#if loading}
    <div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
  {:else}
    <div class="mb-4 max-h-80 min-h-48 overflow-auto rounded-lg border bg-card p-4 font-mono text-xs leading-relaxed">
      {#if output.length === 0}
        <p class="py-12 text-center text-muted-foreground italic">{running ? "Type a command." : "Server offline."}</p>
      {:else}
        {#each output as e}<div class="whitespace-pre-wrap break-all px-1 py-0.5 {e.cls}">{e.text}</div>{/each}
      {/if}
    </div>
    <div class="flex gap-2">
      <Input bind:value={cmd} onkeydown={(e) => e.key === "Enter" && send()} placeholder={running ? "Type a command…" : "Offline"} disabled={!running || sending} class="font-mono" />
      <Button onclick={send} disabled={!cmd.trim() || !running || sending}>{#if sending}<RefreshCwIcon class="size-4 animate-spin" />{:else}<SendIcon class="size-4" />{/if} Send</Button>
    </div>
  {/if}
</div>
