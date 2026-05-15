<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { ServerIcon, PlusIcon, RefreshCwIcon, GlobeIcon, LogOutIcon, TriangleAlertIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import ServerCard from "$lib/components/server-card.svelte";
  import { logViewerState, refreshConnectionState } from "$lib/stores";
  import {
    getApi, getActiveEndpoint, getActiveEndpointId, isConfigured, isAuthenticated,
    clearAuth, resetApi,
  } from "$lib/api";
  import type { MinecraftServer } from "$lib/types";

  let servers = $state<MinecraftServer[]>([]);
  let loading = $state(true);
  let error = $state("");

  const authed = $derived(isAuthenticated());
  const configured = $derived(isConfigured());
  const activeEp = $derived(getActiveEndpoint());

  onMount(() => {
    if (!configured) return;
    if (!authed) return;
    loadServers();
  });

  async function loadServers() {
    loading = true; error = "";
    try { servers = await getApi().get<MinecraftServer[]>("/api/instances") }
    catch (e: unknown) { error = e instanceof Error ? e.message : "Failed" }
    finally { loading = false }
  }

  async function startServer(id: string) {
    try {
      await getApi().post(`/api/instances/${id}/start`);
      servers = servers.map((s) => s.id === id ? { ...s, running: true } : s);
      toast.success("Server started");
    } catch (e: unknown) {
      toast.error("Failed to start server", { description: e instanceof Error ? e.message : "" });
    }
  }
  async function stopServer(id: string) {
    try {
      await getApi().post(`/api/instances/${id}/stop`);
      servers = servers.map((s) => s.id === id ? { ...s, running: false } : s);
      toast.success("Server stopped");
    } catch (e: unknown) {
      toast.error("Failed to stop server", { description: e instanceof Error ? e.message : "" });
    }
  }
  async function deleteServer(id: string) {
    if (!confirm("Delete this server permanently?")) return;
    try {
      await getApi().del(`/api/instances/${id}`);
      servers = servers.filter((s) => s.id !== id);
      toast.success("Server deleted");
    } catch (e: unknown) { toast.error("Failed to delete", { description: e instanceof Error ? e.message : "" }) }
  }
  function viewLogs(id: string) {
    const s = servers.find((s) => s.id === id);
    if (s) logViewerState.set({ open: true, serverId: id, serverName: s.name });
  }
  function logout() {
    const epId = getActiveEndpointId();
    if (epId) clearAuth(epId);
    resetApi(); refreshConnectionState();
    servers = [];
  }

  const rCount = $derived(servers.filter((s) => s.running).length);
</script>

<div class="mx-auto flex flex-col px-6 py-6">
  {#if !configured}
    <div class="flex flex-1 items-center justify-center" style="min-height: calc(100dvh - 3rem)">
      <Card.Root size="sm" class="w-full max-w-md">
        <Card.Header>
          <div class="mb-2 flex items-center gap-2"><ServerIcon class="size-5" /><Card.Title>Welcome</Card.Title></div>
          <Card.Description>No endpoint configured.</Card.Description>
        </Card.Header>
        <Card.Footer><Button variant="outline" class="w-full" onclick={() => goto("/connection")}><GlobeIcon class="size-4" /> Go to Connection</Button></Card.Footer>
      </Card.Root>
    </div>

  {:else if !authed}
    <div class="flex flex-1 items-center justify-center" style="min-height: calc(100dvh - 3rem)">
      <div class="text-center">
        <ServerIcon class="mx-auto mb-3 size-10 text-muted-foreground" />
        <p class="text-sm text-muted-foreground">Authenticate via the sidebar or Connection page.</p>
      </div>
    </div>
  {:else}
    <div class="mb-6 flex items-center justify-between">
      <div>
        <h1 class="text-xl font-semibold">Minecraft Servers</h1>
        <p class="text-sm text-muted-foreground">{activeEp?.name ?? "Servers"}</p>
      </div>
      <div class="flex items-center gap-2">
        <Button variant="outline" size="sm" onclick={loadServers} disabled={loading}>
          <RefreshCwIcon class={loading ? "size-4 animate-spin" : "size-4"} /> Refresh
        </Button>
        <Button size="sm" onclick={() => goto("/servers/new")}><PlusIcon class="size-4" /> New Server</Button>
        <Button variant="ghost" size="sm" onclick={logout}><LogOutIcon class="size-4" /> Logout</Button>
      </div>
    </div>

    <div class="mb-6 grid grid-cols-2 gap-3 sm:grid-cols-2">
      <Card.Root size="sm" class="py-3"><Card.Content class="text-center"><p class="text-2xl font-bold">{servers.length}</p><p class="text-xs text-muted-foreground">Total</p></Card.Content></Card.Root>
      <Card.Root size="sm" class="border-green-200 py-3 dark:border-green-900"><Card.Content class="text-center"><p class="text-2xl font-bold text-green-600 dark:text-green-400">{rCount}</p><p class="text-xs text-muted-foreground">Running</p></Card.Content></Card.Root>
    </div>

    {#if activeEp?.url?.startsWith('http://')}
      <div class="mb-4 flex items-start gap-2 rounded-md border border-amber-500/30 bg-amber-50 p-3 text-sm text-amber-700 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-400">
        <TriangleAlertIcon class="mt-0.5 size-4 shrink-0" />
        <div>
          <span class="font-medium">Not HTTPS</span>
          <p class="mt-0.5 text-xs">Enable IP whitelist on the backend to restrict access when using HTTP.</p>
        </div>
      </div>
    {/if}

    {#if loading}
      <div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
    {:else if servers.length === 0}
      <div class="flex flex-col items-center justify-center gap-4 py-20 text-center">
        <ServerIcon class="size-12 text-muted-foreground" />
        <div><h2 class="text-lg font-medium">No Servers Yet</h2><p class="mt-1 text-sm text-muted-foreground">Create your first server.</p></div>
        <Button onclick={() => goto("/servers/new")}><PlusIcon class="size-4" /> Create Server</Button>
      </div>
    {:else}
      <div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
        {#each servers as server (server.id)}
          <ServerCard {server} onStart={startServer} onStop={stopServer} onDelete={deleteServer} onViewLogs={viewLogs} />
        {/each}
      </div>
    {/if}
  {/if}
</div>
