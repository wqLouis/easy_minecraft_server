<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { ArrowLeftIcon, ServerIcon, PlayIcon, SquareIcon, RefreshCwIcon, UsersIcon, MemoryStickIcon, BoxIcon, DownloadIcon, TerminalIcon, GlobeIcon, Settings2Icon, PuzzleIcon, ExternalLinkIcon, TagIcon, FileArchiveIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Separator } from "$lib/components/ui/separator/index.js";
  import { isAuthenticated, isConfigured, getApi, getActiveEndpoint, getApiKey } from "$lib/api";

  let cfg = $state<Record<string, unknown> | null>(null);
  let st = $state<Record<string, unknown> | null>(null);
  let props = $state<Record<string, string> | null>(null);
  let loading = $state(true), toggling = $state(false);
  let modpackDl = $state(false), modpackGen = $state(false), modpackExists = $state(false), modpackSize = $state("");
  const id = $derived($page.params.id);
  const p = $derived(`/servers/${id}`);
  const on = $derived(st?.running as boolean ?? false);
  const pls = $derived((st?.online_players as string[] | undefined) ?? []);

  // Essential properties shown on dashboard
  const essentialPropKeys = ["difficulty", "server-port", "motd", "max-players", "gamemode", "online-mode", "pvp"];
  const essentialPropLabels: Record<string, string> = {
    "difficulty": "Difficulty",
    "server-port": "Port",
    "motd": "Description",
    "max-players": "Max Players",
    "gamemode": "Gamemode",
    "online-mode": "Online Mode",
    "pvp": "PvP",
  };

  let essentialProps = $state<[string, string][]>([]);

  const nav = [
    { href: "logs", icon: TerminalIcon, label: "Console" },
    { href: "world", icon: GlobeIcon, label: "World" },
    { href: "config", icon: Settings2Icon, label: "Config" },
    { href: "mods", icon: PuzzleIcon, label: "Mods" },
  ];

  onMount(() => { if (!isConfigured() || !isAuthenticated()) goto("/"); fetchS(); });

  async function fetchS() {
    loading = true;
    try {
      const [r, p] = await Promise.all([
        getApi().get<{ config: Record<string, unknown>; status: Record<string, unknown> }>(`/api/instances/${id}`),
        getApi().get<{ properties: Record<string, string> }>(`/api/instances/${id}/properties`).catch(() => ({ properties: {} })),
      ]);
      cfg = r.config; st = r.status; props = p.properties;
      essentialProps = Object.entries(p.properties ?? {}).filter(([k]) => essentialPropKeys.includes(k));
      // Check if a modpack has been generated
      checkModpack();
    }
    catch { goto("/"); } finally { loading = false; }
  }

  async function checkModpack() {
    try {
      const ep = getActiveEndpoint();
      if (!ep) return;
      const res = await fetch(`${ep.url}/api/instances/${id}/mods/modpack/download`, {
        method: "HEAD",
        headers: {
          Authorization: `Bearer ${getApiKey(ep.id)}`,
          "X-Timestamp": Math.floor(Date.now() / 1000).toString(),
          "X-Nonce": crypto.randomUUID(),
        },
      });
      if (res.ok) {
        modpackExists = true;
        const len = res.headers.get("Content-Length");
        if (len) modpackSize = formatBytes(parseInt(len, 10));
      } else {
        modpackExists = false;
      }
    } catch {
      modpackExists = false;
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
  }

  async function toggle() {
    toggling = true;
    try { await getApi().post(on ? `/api/instances/${id}/stop` : `/api/instances/${id}/start`); toast.success(on ? "Stopped" : "Started"); fetchS(); }
    catch (e) { toast.error("Failed", { description: e instanceof Error ? e.message : "" }); } finally { toggling = false; }
  }

  async function downloadModpack() {
    modpackDl = true;
    try {
      const ep = getActiveEndpoint(); if (!ep) return;
      const res = await fetch(`${ep.url}/api/instances/${id}/mods/modpack/download`, {
        headers: {
          Authorization: `Bearer ${getApiKey(ep.id)}`,
          "X-Timestamp": Math.floor(Date.now() / 1000).toString(),
          "X-Nonce": crypto.randomUUID(),
        },
      });
      if (res.status === 404) { toast.error("No modpack generated yet"); modpackExists = false; return; }
      if (!res.ok) throw new Error();
      const blob = await res.blob();
      const m = res.headers.get("Content-Disposition")?.match(/filename="?([^"]+)"?/);
      const a = document.createElement("a"); a.href = URL.createObjectURL(blob); a.download = m?.[1] ?? `${id}.mrpack`; a.click();
    } catch { toast.error("Download failed"); } finally { modpackDl = false; }
  }

  async function generateAndDownload() {
    modpackGen = true;
    try {
      const info = await getApi().post<{ name: string; version: string; size_bytes: number }>(`/api/instances/${id}/mods/modpack`, {
        name: id,
        version: "1.0.0",
        include: [],
      });
      modpackSize = formatBytes(info.size_bytes);
      modpackExists = true;
      toast.success(`Modpack "${info.name}" generated`);
      // Now download it
      await downloadModpack();
    } catch (e) {
      toast.error("Generation failed", { description: e instanceof Error ? e.message : "" });
    } finally { modpackGen = false; }
  }
</script>

<div class="mx-auto max-w-2xl px-6 py-6">
  <button onclick={() => goto("/")} class="mb-4 text-sm text-muted-foreground hover:text-foreground"><ArrowLeftIcon class="inline size-4" /> Back</button>
  {#if loading}
    <div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
  {:else if cfg && st}
    <!-- Header -->
    <div class="mb-4 flex items-start justify-between gap-4">
      <div class="flex items-start gap-3">
        <ServerIcon class="mt-1 size-6 text-muted-foreground" />
        <div><h1 class="text-xl font-semibold">{cfg.name as string}</h1><p class="text-sm text-muted-foreground">{cfg.provider as string} {cfg.version as string}</p></div>
      </div>
      <Badge variant={on ? "default" : "secondary"} class="gap-1.5 px-3 py-1.5"><span class={"size-2 rounded-full " + (on ? "bg-green-500" : "bg-gray-400")}></span>{on ? "Running" : "Stopped"}</Badge>
    </div>

    <!-- Actions -->
    <div class="mb-6 flex items-center gap-2">
      <Button onclick={toggle} disabled={toggling} variant={on ? "destructive" : "default"}>
        {#if toggling}<RefreshCwIcon class="size-4 animate-spin" />{:else if on}<SquareIcon class="size-4" />{:else}<PlayIcon class="size-4" />{/if}{toggling ? "…" : on ? "Stop" : "Start"}
      </Button>
      <Button variant="outline" onclick={fetchS} disabled={loading}><RefreshCwIcon class={loading ? "size-4 animate-spin" : "size-4"} /></Button>
      <a href="/servers/${id}/config" class="ml-auto">
        <Button variant="outline"><Settings2Icon class="size-4" /> Config</Button>
      </a>
    </div>

    <!-- Dashboard grid -->
    <div class="grid gap-4 sm:grid-cols-2">
      <!-- Server Info -->
      <div class="rounded-lg border bg-card p-4">
        <p class="mb-2 text-sm font-medium"><ServerIcon class="inline size-4" /> Server Info</p>
        <div class="space-y-1 text-sm">
          <div class="flex justify-between"><span class="text-muted-foreground">ID</span><span class="font-mono text-xs">{cfg.id as string}</span></div>
          <div class="flex justify-between"><span class="text-muted-foreground">Provider</span><span>{cfg.provider as string}</span></div>
          <div class="flex justify-between"><span class="text-muted-foreground">Version</span><span>{cfg.version as string}</span></div>
        </div>
      </div>

      <!-- Players -->
      <div class="rounded-lg border bg-card p-4">
        <p class="mb-2 text-sm font-medium"><UsersIcon class="inline size-4" /> Players</p>
        {#if on}
          <p class="text-2xl font-bold">{(st?.player_count as number) ?? 0}</p>
          {#if pls.length > 0}<div class="mt-2 flex flex-wrap gap-1">{#each pls as name}<Badge variant="secondary" class="text-xs">{name}</Badge>{/each}</div>{/if}
        {:else}
          <p class="text-xs text-muted-foreground">Server is offline</p>
        {/if}
      </div>

      <!-- Resources summary -->
      <div class="rounded-lg border bg-card p-4">
        <p class="mb-2 text-sm font-medium"><MemoryStickIcon class="inline size-4" /> Resources</p>
        <div class="space-y-1 text-sm">
          <div class="flex justify-between"><span class="text-muted-foreground">Memory</span><span>{cfg.min_memory as string} / {cfg.max_memory as string}</span></div>
          <div class="flex justify-between"><span class="text-muted-foreground">Java</span><span class="font-mono text-xs truncate">{cfg.java_path as string}</span></div>
        </div>
      </div>

      <!-- Modpack — GitHub release style -->
      <div class="rounded-lg border bg-card overflow-hidden">
        {#if modpackExists}
          <!-- GitHub release style -->
          <div class="border-b bg-muted/20 px-4 py-3">
            <div class="flex items-center gap-2">
              <TagIcon class="size-4 text-muted-foreground" />
              <span class="text-sm font-semibold">{id}</span>
              <Badge variant="outline" class="text-[10px] font-mono">v1.0.0</Badge>
            </div>
          </div>
          <div class="p-4">
            <div class="flex items-start justify-between gap-4">
              <div class="space-y-1">
                <h3 class="text-sm font-semibold">Modpack for {cfg.name as string}</h3>
                <p class="text-xs text-muted-foreground">Generated from installed mods/plugins</p>
                <div class="flex items-center gap-3 pt-1 text-xs text-muted-foreground">
                  <span class="flex items-center gap-1"><FileArchiveIcon class="size-3" /> {modpackSize || "—"}</span>
                  <span class="flex items-center gap-1"><BoxIcon class="size-3" /> .mrpack</span>
                </div>
              </div>
              <Button onclick={downloadModpack} disabled={modpackDl} size="sm">
                {#if modpackDl}<RefreshCwIcon class="size-3.5 animate-spin" />{:else}<DownloadIcon class="size-3.5" />{/if}
                {modpackDl ? "…" : "Download"}
              </Button>
            </div>
          </div>
        {:else}
          <!-- No release yet -->
          <div class="p-4">
            <div class="flex items-center gap-2 text-sm font-medium">
              <BoxIcon class="size-4 text-muted-foreground" />
              <span>Modpack</span>
            </div>
            <p class="mt-1 text-xs text-muted-foreground">
              Package your installed mods into a shareable <code>.mrpack</code> file.
            </p>
            <Button onclick={generateAndDownload} disabled={modpackGen} size="sm">
              {#if modpackGen}<RefreshCwIcon class="size-3.5 animate-spin" />{:else}<DownloadIcon class="size-3.5" />{/if}
              {modpackGen ? "Generating…" : "Generate Modpack"}
            </Button>
          </div>
        {/if}
      </div>
    </div>

    <!-- Essential Server Properties -->
    {#if essentialProps.length > 0}
      <div class="mt-6 rounded-lg border bg-card p-4">
        <div class="mb-3 flex items-center justify-between">
          <p class="text-sm font-medium"><Settings2Icon class="inline size-4" /> Server Configuration</p>
          <a href="/servers/${id}/config" class="text-xs text-muted-foreground hover:text-foreground underline">Edit all</a>
        </div>
        <div class="grid gap-x-6 gap-y-2 sm:grid-cols-2 lg:grid-cols-3">
          {#each essentialProps as [k, v]}
            <div class="flex items-baseline gap-2 text-sm">
              <span class="shrink-0 text-muted-foreground">{essentialPropLabels[k] ?? k}:</span>
              <span class="truncate font-mono text-xs">{#if k === "online-mode"}{v === "true" ? "On" : "Off"}{:else if k === "pvp"}{v === "true" ? "Enabled" : "Disabled"}{:else}{v}{/if}</span>
            </div>
          {/each}
        </div>
      </div>
    {:else if props !== null}
      <div class="mt-6 rounded-lg border bg-card p-4 text-center text-sm text-muted-foreground">
        <Settings2Icon class="mx-auto mb-2 size-6" />
        <p>No server.properties yet. Start the server to generate default properties.</p>
      </div>
    {/if}

    <!-- Management Navigation -->
    <div class="mt-8 border-t pt-6">
      <h2 class="mb-4 text-sm font-semibold text-foreground">Management</h2>
      <div class="grid grid-cols-4 gap-3">
        {#each nav as n}
          <a href="{p}/{n.href}" class="flex aspect-square flex-col items-center justify-center gap-1.5 rounded-xl border bg-card shadow-xs transition-all hover:shadow-md hover:border-accent hover:bg-accent/10">
            <n.icon class="size-6 shrink-0 text-muted-foreground" />
            <span class="text-sm font-semibold">{n.label}</span>
          </a>
        {/each}
      </div>
    </div>
  {/if}
</div>
