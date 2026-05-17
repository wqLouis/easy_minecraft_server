<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { ArrowLeftIcon, GlobeIcon, RefreshCwIcon, DownloadIcon, Trash2Icon, FileArchiveIcon, UploadIcon, RotateCcwIcon, InfoIcon, AlertCircleIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Separator } from "$lib/components/ui/separator/index.js";
  import { isAuthenticated, isConfigured, getApi, getActiveEndpoint, getApiKey } from "$lib/api";

  let loading = $state(true);
  let worlds = $state<{ name: string; size_human: string; last_modified: string; region_files: number; player_data_files: number }[]>([]);
  let backups = $state<{ filename: string; size_human: string; created_at: string; worlds_included: string[] }[]>([]);
  let serverRunning = $state(false);
  let backingUp = $state(false);
  const id = $derived($page.params.id);

  onMount(async () => {
    if (!isConfigured() || !isAuthenticated()) { goto("/"); return; }
    await fetchAll();
  });

  async function fetchAll() {
    loading = true;
    try {
      const [wr, br, sr] = await Promise.all([
        getApi().get<{ worlds: typeof worlds }>(`/api/instances/${id}/worlds`).catch(() => ({ worlds: [] })),
        getApi().get<{ backups: typeof backups }>(`/api/instances/${id}/backups`).catch(() => ({ backups: [] })),
        getApi().get<{ status: Record<string, unknown> }>(`/api/instances/${id}`).catch(() => ({ status: {} })),
      ]);
      worlds = wr.worlds ?? [];
      backups = br.backups ?? [];
      serverRunning = (sr.status as Record<string, unknown>)?.running === true;
    } catch { /* ignore */ }
    finally { loading = false; }
  }

  async function downloadWorld(name: string) {
    const ep = getActiveEndpoint(); if (!ep) return;
    const key = getApiKey(ep.id);
    if (!key) return;
    try {
      const res = await fetch(`${ep.url}/api/instances/${id}/worlds/${name}/download`, {
        headers: {
          Authorization: `Bearer ${key}`,
          "X-Timestamp": Math.floor(Date.now() / 1000).toString(),
          "X-Nonce": crypto.randomUUID(),
        },
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const blob = await res.blob();
      const a = document.createElement("a"); a.href = URL.createObjectURL(blob); a.download = `${name}.zip`; a.click();
    } catch (e) { toast.error("Download failed", { description: e instanceof Error ? e.message : "" }); }
  }

  async function deleteWorld(name: string) {
    if (!confirm(`Delete world "${name}"? This cannot be undone.`)) return;
    try {
      await getApi().del(`/api/instances/${id}/worlds/${name}`);
      toast.success(`World "${name}" deleted`);
      fetchAll();
    } catch (e) { toast.error("Delete failed", { description: e instanceof Error ? e.message : "" }); }
  }

  async function backupAll() {
    backingUp = true;
    try {
      await getApi().post(`/api/instances/${id}/worlds/backup`, { include_all: true });
      toast.success("Backup created");
      fetchAll();
    } catch (e) { toast.error("Backup failed", { description: e instanceof Error ? e.message : "" }); }
    finally { backingUp = false; }
  }

  function downloadBackup(filename: string) {
    // Backups are stored server-side; download via the instance's API
    toast.info("Backup download not yet implemented server-side", { description: `File: ${filename}` });
  }
</script>

<div class="mx-auto max-w-4xl px-6 py-6">
  <button onclick={() => goto(`/servers/${id}`)} class="mb-4 text-sm text-muted-foreground hover:text-foreground"><ArrowLeftIcon class="inline size-4" /> Back</button>
  <div class="mb-6 flex items-center justify-between">
    <div class="flex items-center gap-2"><GlobeIcon class="size-5" /><h1 class="text-xl font-semibold">World Management</h1></div>
    <div class="flex items-center gap-2">
      <Button variant="outline" size="sm" onclick={backupAll} disabled={backingUp || worlds.length === 0}>
        {#if backingUp}<RefreshCwIcon class="size-4 animate-spin" />{:else}<FileArchiveIcon class="size-4" />{/if} Backup All
      </Button>
      <Button variant="outline" size="sm" onclick={fetchAll} disabled={loading}>
        <RefreshCwIcon class={loading ? "size-4 animate-spin" : "size-4"} /> Refresh
      </Button>
    </div>
  </div>

  {#if loading}
    <div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
  {:else}
    <!-- World list -->
    <Card.Root size="sm" class="mb-6">
      <Card.Header>
        <Card.Title>Worlds</Card.Title>
        <Card.Description>Minecraft world directories detected in the server folder.</Card.Description>
      </Card.Header>
      <Card.Content>
        {#if worlds.length === 0}
          <p class="py-8 text-center text-sm text-muted-foreground">No worlds found.</p>
        {:else}
          <div class="overflow-x-auto">
            <table class="w-full text-sm">
              <thead>
                <tr class="border-b text-left text-xs text-muted-foreground">
                  <th class="px-3 py-2 font-medium">Name</th>
                  <th class="px-3 py-2 font-medium">Size</th>
                  <th class="px-3 py-2 font-medium">Regions</th>
                  <th class="px-3 py-2 font-medium">Players</th>
                  <th class="px-3 py-2 font-medium text-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {#each worlds as w}
                  <tr class="border-b last:border-0 hover:bg-muted/30">
                    <td class="px-3 py-3"><div class="flex items-center gap-2"><GlobeIcon class="size-4 text-muted-foreground" /><span class="font-medium">{w.name}</span></div></td>
                    <td class="px-3 py-3 text-muted-foreground">{w.size_human}</td>
                    <td class="px-3 py-3 text-muted-foreground">{w.region_files}</td>
                    <td class="px-3 py-3 text-muted-foreground">{w.player_data_files}</td>
                    <td class="px-3 py-3 text-right"><div class="flex items-center justify-end gap-1">
                      <Button variant="ghost" size="icon-xs" onclick={() => downloadWorld(w.name)} title="Download"><DownloadIcon class="size-3" /></Button>
                      <Button variant="ghost" size="icon-xs" class="text-destructive hover:text-destructive" onclick={() => deleteWorld(w.name)} title="Delete" disabled={serverRunning}><Trash2Icon class="size-3" /></Button>
                    </div></td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </Card.Content>
    </Card.Root>

    <!-- Backups -->
    <Card.Root size="sm">
      <Card.Header>
        <Card.Title class="flex items-center gap-2"><FileArchiveIcon class="size-4" /> Backups</Card.Title>
        <Card.Description>World backups stored on the server.</Card.Description>
      </Card.Header>
      <Card.Content>
        {#if backups.length === 0}
          <p class="py-8 text-center text-sm text-muted-foreground">No backups yet. Click "Backup All" to create one.</p>
        {:else}
          <div class="overflow-x-auto">
            <table class="w-full text-sm">
              <thead>
                <tr class="border-b text-left text-xs text-muted-foreground">
                  <th class="px-3 py-2 font-medium">File</th>
                  <th class="px-3 py-2 font-medium">Size</th>
                  <th class="px-3 py-2 font-medium">Worlds</th>
                  <th class="px-3 py-2 font-medium">Created</th>
                  <th class="px-3 py-2 font-medium text-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {#each backups as b}
                  <tr class="border-b last:border-0 hover:bg-muted/30">
                    <td class="px-3 py-3 font-mono text-xs">{b.filename}</td>
                    <td class="px-3 py-3 text-muted-foreground">{b.size_human}</td>
                    <td class="px-3 py-3 text-muted-foreground">{b.worlds_included.join(", ")}</td>
                    <td class="px-3 py-3 text-xs text-muted-foreground">{b.created_at}</td>
                    <td class="px-3 py-3 text-right"><Button variant="ghost" size="icon-xs" onclick={() => downloadBackup(b.filename)} title="Download"><DownloadIcon class="size-3" /></Button></td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </Card.Content>
    </Card.Root>

    {#if serverRunning}
      <div class="mt-4 flex items-start gap-2 rounded-lg border border-amber-500/30 bg-amber-50 p-3 text-xs text-amber-700 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-400">
        <AlertCircleIcon class="mt-0.5 size-4 shrink-0" />
        <span>Server is running. Stop it to delete or reset worlds.</span>
      </div>
    {/if}
  {/if}
</div>
