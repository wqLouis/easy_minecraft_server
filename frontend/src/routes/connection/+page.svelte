<script lang="ts">
  import { onMount } from "svelte";
  import { toast } from "svelte-sonner";
  import { RefreshCwIcon, CheckCircleIcon, GlobeIcon, PlusIcon, Trash2Icon, CheckIcon, ServerIcon, KeyIcon, LogOutIcon, TriangleAlertIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { refreshConnectionState } from "$lib/stores";
  import {
    getEndpoints, addEndpoint, removeEndpoint, setActiveEndpoint, getActiveEndpointId,
    getActiveEndpoint, getApiKey, setApiKey, clearAuth, isAuthenticated, getApi, resetApi,
    type Endpoint,
  } from "$lib/api";

  let endpoints = $state<Endpoint[]>([]);
  let activeId = $state<string | null>(null);
  let selectedId = $state<string | null>(null);
  let newName = $state(""), newUrl = $state("");
  let apiKey = $state(""), authTesting = $state(false);

  onMount(() => {
    endpoints = getEndpoints();
    activeId = getActiveEndpointId();
    if (!selectedId) selectedId = activeId ?? endpoints[0]?.id ?? null;
  });

  const selected = $derived(endpoints.find((e) => e.id === selectedId) ?? null);
  const authed = $derived(selectedId ? isAuthenticated(selectedId) : false);

  function refresh() {
    endpoints = getEndpoints();
    activeId = getActiveEndpointId();
    refreshConnectionState();
  }

  function add() {
    const name = newName.trim(), url = newUrl.trim();
    if (!name || !url) return;
    addEndpoint(name, url);
    newName = ""; newUrl = "";
    refresh();
    selectedId = endpoints.length > 0 ? endpoints[endpoints.length - 1].id : null;
    toast.success(`Endpoint "${name}" added`);
  }

  function remove(id: string) {
    const ep = endpoints.find((e) => e.id === id);
    if (!ep || !confirm(`Remove "${ep.name}"?`)) return;
    removeEndpoint(id);
    refresh();
    if (selectedId === id) selectedId = endpoints[0]?.id ?? null;
    toast.success(`"${ep.name}" removed`);
  }

  function setActive(id: string) {
    setActiveEndpoint(id);
    refresh();
    toast.success("Active endpoint switched");
  }

  async function authenticate() {
    if (!selected || !apiKey.trim()) return;
    authTesting = true;
    setApiKey(apiKey.trim(), selected.id);
    resetApi();
    try {
      const me = await getApi().get<{ user: { email: string; is_sudoer: boolean } }>("/api/auth/me");
      toast.success(`Authenticated as ${me.user.email}${me.user.is_sudoer ? " (sudoer)" : ""}`);
      refresh();
    } catch (e: unknown) {
      clearAuth(selected.id);
      toast.error("Auth failed", { description: e instanceof Error ? e.message : "" });
    } finally { authTesting = false }
  }

  function logout() {
    if (!selected) return;
    clearAuth(selected.id);
    resetApi();
    refresh();
    toast.success("Logged out");
  }
</script>

<div class="mx-auto max-w-3xl px-6 py-6">
  <div class="mb-6">
    <h1 class="text-xl font-semibold">Connection</h1>
    <p class="text-sm text-muted-foreground">Manage endpoints and authentication. API keys are in-memory only.</p>
  </div>

  <div class="grid gap-6 lg:grid-cols-5">
    <div class="space-y-3 lg:col-span-2">
      <Card.Root size="sm">
        <Card.Header><Card.Title class="flex items-center gap-2"><ServerIcon class="size-4" /> Endpoints</Card.Title></Card.Header>
        <Card.Content class="grid gap-2">
          {#each endpoints as ep (ep.id)}
            <div role="button" tabindex="0"
              class="flex cursor-pointer items-center gap-2 rounded-md border px-3 py-2 text-sm transition-colors {selectedId === ep.id ? 'border-primary bg-accent/30' : 'hover:bg-accent/20'}"
              onclick={() => { selectedId = ep.id; apiKey = "" }}
              onkeydown={(e) => e.key === "Enter" && (selectedId = ep.id)}>
              <div class="shrink-0">
                {#if activeId === ep.id}
                  <CheckCircleIcon class="size-4 text-green-500" />
                {:else}
                  <div class="size-4 rounded-full border-2 border-muted-foreground/30"></div>
                {/if}
              </div>
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-1.5">
                  <span class="truncate font-medium">{ep.name}</span>
                  {#if activeId === ep.id}<Badge variant="secondary" class="text-[10px]">active</Badge>{/if}
                </div>
                <p class="truncate text-xs text-muted-foreground">{ep.url}</p>
              </div>
              <div class="size-2 shrink-0 rounded-full {isAuthenticated(ep.id) ? 'bg-green-500' : 'bg-gray-300'}"></div>
            </div>
          {:else}
            <p class="py-4 text-center text-sm text-muted-foreground">No endpoints yet.</p>
          {/each}
        </Card.Content>
      </Card.Root>

      <Card.Root size="sm">
        <Card.Header><Card.Title class="flex items-center gap-2"><PlusIcon class="size-4" /> Add Endpoint</Card.Title></Card.Header>
        <Card.Content class="grid gap-3"><Input placeholder="Name" bind:value={newName} /><Input type="url" placeholder="http://192.168.1.100:3000" bind:value={newUrl} /></Card.Content>
        <Card.Footer><Button onclick={add} disabled={!newName.trim() || !newUrl.trim()} class="w-full"><PlusIcon class="size-4" /> Add</Button></Card.Footer>
      </Card.Root>
    </div>

    <div class="space-y-4 lg:col-span-3">
      {#if selected}
        <Card.Root size="sm">
          <Card.Content class="flex flex-wrap items-center justify-between gap-2">
            <div class="flex items-center gap-2">
              <GlobeIcon class="size-4 text-muted-foreground" />
              <span class="text-sm font-medium">{selected.name}</span>
              <span class="text-xs text-muted-foreground">{selected.url}</span>
            </div>
            <div class="flex items-center gap-1">
              <Button variant="outline" size="xs" onclick={() => setActive(selected.id)} disabled={activeId === selected.id}><CheckIcon class="size-3" /> Set Active</Button>
              <Button variant="ghost" size="icon-xs" class="text-destructive hover:text-destructive" onclick={() => remove(selected.id)}><Trash2Icon class="size-3" /></Button>
            </div>
          </Card.Content>
        </Card.Root>

        {#if selected?.url?.startsWith('http://')}
          <div class="flex items-start gap-2 rounded-md border border-amber-500/30 bg-amber-50 p-3 text-sm text-amber-700 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-400">
            <TriangleAlertIcon class="mt-0.5 size-4 shrink-0" />
            <div>
              <span class="font-medium">Insecure connection</span>
              <p class="mt-0.5 text-xs">This endpoint uses HTTP, not HTTPS. Enable IP whitelist on the backend to restrict access by IP address.</p>
            </div>
          </div>
        {/if}

        <Card.Root size="sm">
          <Card.Header>
            <Card.Title class="flex items-center gap-2"><KeyIcon class="size-4" /> Authentication</Card.Title>
            <Card.Description>API key is in-memory only. Lost on page reload.</Card.Description>
          </Card.Header>
          <Card.Content class="grid gap-3">
            {#if !authed}
              <Input type="password" placeholder="Paste your API key" bind:value={apiKey} disabled={authTesting} />
            {:else}
              <div class="flex items-center gap-2 rounded-md border border-green-300 bg-green-50 p-3 text-sm text-green-700 dark:border-green-800 dark:bg-green-950 dark:text-green-400">
                <CheckCircleIcon class="size-4 shrink-0" /> Authenticated
              </div>
            {/if}
          </Card.Content>
          <Card.Footer>
            {#if authed}
              <Button variant="outline" onclick={logout}><LogOutIcon class="size-4" /> Logout</Button>
            {:else}
              <Button onclick={authenticate} disabled={!apiKey.trim() || authTesting}>
                {#if authTesting}<RefreshCwIcon class="size-4 animate-spin" />{:else}<KeyIcon class="size-4" />{/if}
                Authenticate
              </Button>
            {/if}
          </Card.Footer>
        </Card.Root>

      {:else}
        <div class="flex items-center justify-center py-20 text-center">
          <div><GlobeIcon class="mx-auto mb-2 size-10 text-muted-foreground" /><p class="text-sm text-muted-foreground">Select an endpoint.</p></div>
        </div>
      {/if}
    </div>
  </div>
</div>
