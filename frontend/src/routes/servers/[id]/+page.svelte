<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { ArrowLeftIcon, ServerIcon, PlayIcon, SquareIcon, RefreshCwIcon, UsersIcon, MemoryStickIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import { Separator } from "$lib/components/ui/separator/index.js";
  import { isAuthenticated, isConfigured, getApi } from "$lib/api";

  let config = $state<Record<string, unknown> | null>(null);
  let status = $state<Record<string, unknown> | null>(null);
  let loading = $state(true);
  let error = $state("");
  let toggling = $state(false);

  const id = $derived($page.params.id);

  onMount(() => {
    if (!isConfigured() || !isAuthenticated()) { goto("/"); return }
    fetchServer();
  });

  async function fetchServer() {
    loading = true; error = "";
    try {
      const res = await getApi().get<{ config: Record<string, unknown>; status: Record<string, unknown> }>(`/api/instances/${id}`);
      config = res.config;
      status = res.status;
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : "Failed to load";
      if (error.includes("404") || error.includes("403")) goto("/");
    } finally { loading = false }
  }

  const running = $derived(status?.running === true);
  const players = $derived((status?.online_players as string[]) ?? []);
  const playerCount = $derived(status?.player_count ?? 0);

  async function toggle() {
    toggling = true;
    try {
      await getApi().post(running ? `/api/instances/${id}/stop` : `/api/instances/${id}/start`);
      toast.success(running ? "Server stopped" : "Server started");
      fetchServer();
    } catch (e: unknown) {
      toast.error("Failed", { description: e instanceof Error ? e.message : "" });
    } finally { toggling = false }
  }
</script>

<div class="mx-auto max-w-2xl px-6 py-6">
  <button onclick={() => goto("/")} class="mb-4 flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground">
    <ArrowLeftIcon class="size-4" /> Back to Servers
  </button>

  {#if loading}
    <div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>

  {:else if error}
    <div class="flex flex-col items-center gap-4 py-20 text-center">
      <p class="text-sm text-muted-foreground">{error}</p>
      <Button onclick={fetchServer}>Retry</Button>
    </div>

  {:else if config && status}
    <div class="mb-6 flex items-start justify-between gap-4">
      <div class="flex items-start gap-3">
        <ServerIcon class="mt-1 size-6 text-muted-foreground" />
        <div>
          <h1 class="text-xl font-semibold">{config.name as string}</h1>
          <p class="mt-0.5 text-sm text-muted-foreground">{config.provider as string} {config.version as string}</p>
        </div>
      </div>
      <div class="flex items-center gap-2">
        <Badge variant={running ? "default" : "secondary"} class="gap-1.5 px-3 py-1.5 text-sm">
          <span class={"size-2 rounded-full " + (running ? "bg-green-500" : "bg-gray-400")}></span>
          {running ? "Running" : "Stopped"}
        </Badge>
      </div>
    </div>

    <div class="mb-6 flex items-center gap-2">
      <Button onclick={toggle} disabled={toggling} variant={running ? "destructive" : "default"}>
        {#if toggling}
          <RefreshCwIcon class="size-4 animate-spin" />
        {:else if running}
          <SquareIcon class="size-4" />
        {:else}
          <PlayIcon class="size-4" />
        {/if}
        {toggling ? "Please wait…" : running ? "Stop Server" : "Start Server"}
      </Button>
      <Button variant="outline" onclick={fetchServer} disabled={loading}>
        <RefreshCwIcon class={loading ? "size-4 animate-spin" : "size-4"} /> Refresh
      </Button>
    </div>

    <div class="grid gap-4 sm:grid-cols-2">
      <!-- Info -->
      <Card.Root size="sm">
        <Card.Header><Card.Title>Details</Card.Title></Card.Header>
        <Card.Content class="grid gap-3 text-sm">
          <div class="flex justify-between"><span class="text-muted-foreground">ID</span><span class="font-mono text-xs">{config.id as string}</span></div>
          <div class="flex justify-between"><span class="text-muted-foreground">Provider</span><span>{config.provider as string}</span></div>
          <div class="flex justify-between"><span class="text-muted-foreground">Version</span><span>{config.version as string}</span></div>
          <div class="flex justify-between"><span class="text-muted-foreground">Directory</span><span class="truncate text-xs font-mono">{config.server_dir as string}</span></div>
        </Card.Content>
      </Card.Root>

      <!-- Players -->
      <Card.Root size="sm">
        <Card.Header><Card.Title class="flex items-center gap-2"><UsersIcon class="size-4" /> Players</Card.Title></Card.Header>
        <Card.Content class="text-sm">
          {#if running}
            <p class="mb-2 text-2xl font-bold">{playerCount}</p>
            {#if players.length > 0}
              <div class="flex flex-wrap gap-1">
                {#each players as name}<Badge variant="secondary" class="text-xs">{name}</Badge>{/each}
              </div>
            {:else}
              <p class="text-xs text-muted-foreground">No players online.</p>
            {/if}
          {:else}
            <div class="flex flex-col items-center py-4 text-muted-foreground">
              <UsersIcon class="mb-1 size-6" />
              <p class="text-xs">Server is offline</p>
            </div>
          {/if}
        </Card.Content>
      </Card.Root>

      <!-- Resources -->
      <Card.Root size="sm">
        <Card.Header><Card.Title class="flex items-center gap-2"><MemoryStickIcon class="size-4" /> Resources</Card.Title></Card.Header>
        <Card.Content class="grid gap-3 text-sm">
          <div class="flex justify-between"><span class="text-muted-foreground">Min Memory</span><span>{config.min_memory as string}</span></div>
          <div class="flex justify-between"><span class="text-muted-foreground">Max Memory</span><span>{config.max_memory as string}</span></div>
          <div class="flex justify-between"><span class="text-muted-foreground">Java</span><span class="truncate text-xs font-mono">{config.java_path as string}</span></div>
          {#if (config.jvm_args as string[])?.length}
            <Separator />
            <div><span class="text-muted-foreground text-xs">JVM Args</span><p class="mt-1 text-xs font-mono">{(config.jvm_args as string[]).join(" ")}</p></div>
          {/if}
        </Card.Content>
      </Card.Root>
    </div>
  {/if}
</div>
