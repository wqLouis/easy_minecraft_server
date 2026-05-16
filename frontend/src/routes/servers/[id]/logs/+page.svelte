<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { ArrowLeftIcon, RefreshCwIcon, SendHorizonalIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { isAuthenticated, isConfigured, getApi } from "$lib/api";

  let logs = $state<string[]>([]);
  let loading = $state(true);
  let cmd = $state("");
  let sending = $state(false);
  let refreshing = $state(false);
  let container = $state<HTMLDivElement | null>(null);
  let autoScroll = $state(true);
  const id = $derived($page.params.id);
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  onMount(() => {
    if (!isConfigured() || !isAuthenticated()) { goto("/"); return; }
    fetchLogs();
    pollTimer = setInterval(pollLogs, 3000);
  });

  onDestroy(() => {
    if (pollTimer) clearInterval(pollTimer);
  });

  $effect(() => {
    if (container && autoScroll) {
      requestAnimationFrame(() => {
        container!.scrollTop = container!.scrollHeight;
      });
    }
  });

  async function fetchLogs() {
    loading = true;
    try { logs = (await getApi().get<{ logs: string[] }>(`/api/instances/${id}/logs?tail=200`)).logs; }
    catch { goto("/"); }
    finally { loading = false }
  }

  async function pollLogs() {
    try { logs = (await getApi().get<{ logs: string[] }>(`/api/instances/${id}/logs?tail=200`)).logs; }
    catch { /* silent */ }
  }

  async function send() {
    const c = cmd.trim(); if (!c || sending) return;
    sending = true;
    logs = [...logs, `> ${c}`];
    cmd = "";
    requestAnimationFrame(() => { if (container) container.scrollTop = container.scrollHeight; });
    try {
      await getApi().post(`/api/instances/${id}/command`, { command: c });
      toast.success("Command sent");
      setTimeout(pollLogs, 500);
    } catch (e) { toast.error("Failed", { description: e instanceof Error ? e.message : "" }); }
    sending = false;
  }

  function handleScroll() {
    if (!container) return;
    const threshold = 40;
    autoScroll = container.scrollHeight - container.scrollTop - container.clientHeight < threshold;
  }
</script>

<div class="mx-auto max-w-5xl px-4 py-6">
  <button onclick={() => goto(`/servers/${id}`)} class="mb-4 text-sm text-muted-foreground hover:text-foreground"><ArrowLeftIcon class="inline size-4" /> Back</button>

  <div class="overflow-hidden rounded-xl border bg-card shadow-lg">
    {#if loading}
      <div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
    {:else}
      <!-- Toolbar -->
      <div class="flex items-center justify-between border-b bg-muted/30 px-4 py-2">
        <div class="flex items-center gap-2.5">
          <span class="text-xs font-semibold tracking-wider text-muted-foreground">Console</span>
          <span class="relative flex size-2">
            <span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400/60 opacity-75"></span>
            <span class="relative inline-flex size-2 rounded-full bg-emerald-500"></span>
          </span>
        </div>
        <Button variant="ghost" size="sm" onclick={async () => { refreshing = true; await pollLogs(); refreshing = false; }} class="h-6 text-muted-foreground hover:text-foreground">
          <RefreshCwIcon class={refreshing ? "size-3 animate-spin" : "size-3"} />
        </Button>
      </div>

      <!-- Output -->
      <div
        bind:this={container}
        onscroll={handleScroll}
        class="h-[55dvh] overflow-y-auto bg-background p-4 font-mono text-sm leading-relaxed scrollbar-thin"
      >
        {#if logs.length === 0}
          <p class="py-12 text-center text-muted-foreground italic">No logs</p>
        {:else}
          {#each logs as line}
            <div class="whitespace-pre-wrap break-all leading-6">
              {#if line.startsWith('> ')}
                <span class="text-foreground">{line}</span>
              {:else if /ERROR|FATAL|Exception/.test(line)}
                <span class="text-destructive">{line}</span>
              {:else if /WARN/.test(line)}
                <span class="text-amber-500 dark:text-amber-400">{line}</span>
              {:else if /^\d{2}:\d{2}:\d{2}/.test(line)}
                <span><span class="text-muted-foreground">[{line.slice(0, 8)}]</span>{line.slice(8)}</span>
              {:else}
                {line}
              {/if}
            </div>
          {/each}
        {/if}
      </div>

      <!-- Input bar -->
      <div class="flex items-center gap-2 border-t bg-muted/20 px-4 py-3">
        <input
          bind:value={cmd}
          onkeydown={(e) => e.key === "Enter" && send()}
          placeholder="Type a command…"
          disabled={sending}
          class="min-w-0 flex-1 border-none bg-transparent p-0 font-mono text-sm text-foreground outline-none ring-0 placeholder:text-muted-foreground/40"
        />
        <Button
          onclick={send}
          disabled={!cmd.trim() || sending}
          size="sm"
          variant="ghost"
          class="shrink-0 text-muted-foreground hover:text-foreground disabled:opacity-30"
        >
          <SendHorizonalIcon class="size-4" />
        </Button>
      </div>
    {/if}
  </div>
</div>

<style>
  .scrollbar-thin::-webkit-scrollbar {
    width: 6px;
  }
  .scrollbar-thin::-webkit-scrollbar-track {
    background: transparent;
  }
  .scrollbar-thin::-webkit-scrollbar-thumb {
    background: hsl(var(--border));
    border-radius: 3px;
  }
  .scrollbar-thin::-webkit-scrollbar-thumb:hover {
    background: hsl(var(--muted-foreground) / 0.3);
  }
</style>
