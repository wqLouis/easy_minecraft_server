<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { ArrowLeftIcon, ServerIcon, PlayIcon, SquareIcon, RefreshCwIcon, UsersIcon, MemoryStickIcon, BoxIcon, DownloadIcon, FileTextIcon, TerminalIcon, GlobeIcon, Settings2Icon, PuzzleIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { isAuthenticated, isConfigured, getApi, getActiveEndpoint, getApiKey } from "$lib/api";

  let cfg = $state<Record<string, unknown> | null>(null);
  let st = $state<Record<string, unknown> | null>(null);
  let loading = $state(true), toggling = $state(false), modpackDl = $state(false), noMp = $state(false);
  const id = $derived($page.params.id);
  const p = $derived(`/servers/${id}`);
  const on = $derived(st?.running as boolean ?? false);
  const pls = $derived((st?.online_players as string[] | undefined) ?? []);
  const nav = [
    { href: "logs", icon: FileTextIcon, label: "Logs" },
    { href: "console", icon: TerminalIcon, label: "Console" },
    { href: "world", icon: GlobeIcon, label: "World" },
    { href: "config", icon: Settings2Icon, label: "Config" },
    { href: "mods", icon: PuzzleIcon, label: "Mods" },
  ];

  onMount(() => { if (!isConfigured() || !isAuthenticated()) goto("/"); fetchS(); });

  async function fetchS() {
    loading = true;
    try { const r = await getApi().get<{ config: Record<string, unknown>; status: Record<string, unknown> }>(`/api/instances/${id}`); cfg = r.config; st = r.status; }
    catch { goto("/"); } finally { loading = false; }
  }

  async function toggle() {
    toggling = true;
    try { await getApi().post(on ? `/api/instances/${id}/stop` : `/api/instances/${id}/start`); toast.success(on ? "Stopped" : "Started"); fetchS(); }
    catch (e) { toast.error("Failed", { description: e instanceof Error ? e.message : "" }); } finally { toggling = false; }
  }

  async function dl() {
    modpackDl = true; noMp = false;
    try {
      const ep = getActiveEndpoint(); if (!ep) return;
      const res = await fetch(`${ep.url}/api/instances/${id}/mods/modpack/download`, { headers: { Authorization: `Bearer ${getApiKey(ep.id)}` } });
      if (res.status === 404) { noMp = true; toast.error("No modpack"); return; }
      if (!res.ok) throw new Error();
      const blob = await res.blob();
      const m = res.headers.get("Content-Disposition")?.match(/filename="?([^"]+)"?/);
      const a = document.createElement("a"); a.href = URL.createObjectURL(blob); a.download = m?.[1] ?? `${id}.mrpack`; a.click();
    } catch { toast.error("Download failed"); } finally { modpackDl = false; }
  }
</script>

<div class="mx-auto max-w-2xl px-6 py-6">
  <button onclick={() => goto("/")} class="mb-4 text-sm text-muted-foreground hover:text-foreground"><ArrowLeftIcon class="inline size-4" /> Back</button>
  {#if loading}
    <div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
  {:else if cfg && st}
    <div class="mb-4 flex items-start justify-between gap-4">
      <div class="flex items-start gap-3">
        <ServerIcon class="mt-1 size-6 text-muted-foreground" />
        <div><h1 class="text-xl font-semibold">{cfg.name as string}</h1><p class="text-sm text-muted-foreground">{cfg.provider as string} {cfg.version as string}</p></div>
      </div>
      <Badge variant={on ? "default" : "secondary"} class="gap-1.5 px-3 py-1.5"><span class={"size-2 rounded-full " + (on ? "bg-green-500" : "bg-gray-400")}></span>{on ? "Running" : "Stopped"}</Badge>
    </div>
    <div class="mb-4 flex items-center gap-2">
      <Button onclick={toggle} disabled={toggling} variant={on ? "destructive" : "default"}>
        {#if toggling}<RefreshCwIcon class="size-4 animate-spin" />{:else if on}<SquareIcon class="size-4" />{:else}<PlayIcon class="size-4" />{/if}{toggling ? "…" : on ? "Stop" : "Start"}
      </Button>
      <Button variant="outline" onclick={fetchS} disabled={loading}><RefreshCwIcon class={loading ? "size-4 animate-spin" : "size-4"} /></Button>
    </div>
    <div class="grid gap-4 sm:grid-cols-2">
      <div class="rounded-lg border bg-card p-4">
        <p class="mb-2 text-sm font-medium">Details</p>
        <div class="space-y-1 text-sm">{cfg.id as string}<br />{cfg.provider as string} {cfg.version as string}</div>
      </div>
      <div class="rounded-lg border bg-card p-4">
        <p class="mb-2 text-sm font-medium"><UsersIcon class="inline size-4" /> Players</p>
        {#if on}
          <p class="text-2xl font-bold">{(st?.player_count as number) ?? 0}</p>
          {#if pls.length > 0}<div class="flex flex-wrap gap-1">{#each pls as name}<Badge variant="secondary" class="text-xs">{name}</Badge>{/each}</div>{/if}
        {:else}<p class="text-xs text-muted-foreground">Offline</p>{/if}
      </div>
      <div class="rounded-lg border bg-card p-4">
        <p class="mb-2 text-sm font-medium"><MemoryStickIcon class="inline size-4" /> Resources</p>
        <div class="space-y-1 text-sm">{cfg.min_memory as string} / {cfg.max_memory as string}<br />{cfg.java_path as string}</div>
      </div>
      <div class="rounded-lg border bg-card p-4">
        <p class="mb-2 text-sm font-medium"><BoxIcon class="inline size-4" /> Modpack</p>
        <Button onclick={dl} disabled={modpackDl} class="w-full">{#if modpackDl}<RefreshCwIcon class="size-4 animate-spin" />{:else}<DownloadIcon class="size-4" />{/if} Download</Button>
        {#if noMp}<p class="mt-1 text-xs text-muted-foreground">None available</p>{/if}
      </div>
    </div>
    <div class="mt-6">
      <p class="mb-2 text-xs font-medium text-muted-foreground">Management</p>
      <div class="grid grid-cols-5 gap-2">
        {#each nav as n}<a href="{p}/{n.href}" class="flex flex-col items-center gap-1 rounded-lg border p-2 text-center text-xs transition-colors hover:bg-accent/20"><n.icon class="size-5 text-muted-foreground" /><span class="font-medium">{n.label}</span></a>{/each}
      </div>
    </div>
  {/if}
</div>
