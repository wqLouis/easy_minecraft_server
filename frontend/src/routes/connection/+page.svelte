<script lang="ts">
  import { onMount } from "svelte";
  import { toast } from "svelte-sonner";
  import { RefreshCwIcon, CheckCircleIcon, GlobeIcon, PlusIcon, Trash2Icon, KeyIcon, LogOutIcon, XIcon, TriangleAlertIcon, GripVerticalIcon, ArrowRightIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { refreshConnectionState } from "$lib/stores";
  import { getEndpoints, addEndpoint, removeEndpoint, setActiveEndpoint, clearActiveEndpoint, getActiveEndpoint, setApiKey, clearAuth, isAuthenticated, getApi, resetApi, type Endpoint } from "$lib/api";

  let eps = $state<Endpoint[]>([]);
  let active = $state<Endpoint | null>(null);
  let nName = $state(""), nUrl = $state("");
  let key = $state(""), testing = $state(false), hover = $state(false);

  function ref() { eps = getEndpoints(); active = getActiveEndpoint(); refreshConnectionState(); }
  onMount(ref);

  const authed = $derived(active ? isAuthenticated(active.id) : false);

  function add() {
    if (!nName.trim() || !nUrl.trim()) return;
    addEndpoint(nName.trim(), nUrl.trim());
    nName = ""; nUrl = "";
    ref();
    toast.success("Added");
  }

  function rm(id: string) {
    const e = eps.find((x) => x.id === id);
    if (!e || !confirm(`Remove "${e.name}"?`)) return;
    removeEndpoint(id); ref(); toast.success(`Removed "${e.name}"`);
  }

  function act(e: Endpoint) { setActiveEndpoint(e.id); ref(); key = ""; toast.success(`"${e.name}" activated`); }

  function deact() {
    if (!active) return;
    clearAuth(active.id); resetApi(); clearActiveEndpoint(); ref(); toast.success("Disconnected");
  }

  function dragStart(ev: DragEvent, e: Endpoint) { ev.dataTransfer?.setData("text/plain", e.id); if (ev.dataTransfer) ev.dataTransfer.effectAllowed = "move"; }
  function dragOver(ev: DragEvent) { ev.preventDefault(); if (ev.dataTransfer) ev.dataTransfer.dropEffect = "move"; hover = true; }
  function dragLeave() { hover = false; }
  function drop(ev: DragEvent) {
    ev.preventDefault(); hover = false;
    const id = ev.dataTransfer?.getData("text/plain");
    const e = id ? eps.find((x) => x.id === id) : null;
    if (e) act(e);
  }

  async function auth() {
    if (!active || !key.trim()) return;
    testing = true;
    setApiKey(key.trim(), active.id); resetApi();
    try {
      const me = await getApi().get<{ user: { username: string; is_sudoer: boolean } }>("/api/auth/me");
      toast.success(`Authenticated as ${me.user.username}${me.user.is_sudoer ? " (sudo)" : ""}`); ref();
    } catch { clearAuth(active.id); toast.error("Auth failed"); } finally { testing = false; }
  }

  function logout() { if (!active) return; clearAuth(active.id); resetApi(); ref(); toast.success("Logged out"); }
</script>

<div class="mx-auto flex max-w-4xl gap-6 px-6 py-6">
  <div class="w-72 shrink-0 space-y-4">
    <div><h2 class="mb-1 text-sm font-semibold">Available</h2><p class="text-xs text-muted-foreground">Drag to the slot to activate.</p></div>
    <div class="space-y-2">
      {#each eps as e (e.id)}
        <div draggable="true" ondragstart={(ev) => dragStart(ev, e)}
          class="flex cursor-grab items-center gap-2 rounded-lg border bg-card p-3 text-sm transition-all hover:bg-accent/20 active:cursor-grabbing active:shadow-md {active?.id === e.id ? 'opacity-50' : ''}">
          <GripVerticalIcon class="size-4 shrink-0 text-muted-foreground" />
          <div class="min-w-0 flex-1"><span class="font-medium">{e.name}</span><p class="truncate text-xs text-muted-foreground">{e.url}</p></div>
          <div class="flex items-center gap-1">
            <div class="size-2 rounded-full {isAuthenticated(e.id) ? 'bg-green-500' : 'bg-gray-300'}"></div>
            <button onclick={() => rm(e.id)} class="rounded p-0.5 text-muted-foreground hover:text-destructive"><Trash2Icon class="size-3" /></button>
          </div>
        </div>
      {:else}
        <div class="rounded-lg border bg-card p-6 text-center text-xs text-muted-foreground"><GlobeIcon class="mx-auto mb-1 size-6" /><p>No connections yet.</p></div>
      {/each}
    </div>
    <div class="space-y-2 rounded-lg border bg-card p-3">
      <Input placeholder="Name" bind:value={nName} />
      <Input type="url" placeholder="http://192.168.1.100:3000" bind:value={nUrl} />
      <Button onclick={add} disabled={!nName.trim() || !nUrl.trim()} class="w-full"><PlusIcon class="size-4" /> Add</Button>
    </div>
  </div>

  <div class="flex-1 space-y-4">
    <div ondrop={drop} ondragover={dragOver} ondragleave={dragLeave}
      class="flex min-h-24 items-center justify-center rounded-xl border-2 transition-colors {hover ? 'border-primary bg-primary/5' : 'border-dashed border-muted-foreground/30'} {active ? 'border-solid border-green-500' : ''}">
      {#if active}
        <div class="flex w-full items-center justify-between px-4 py-3">
          <div class="flex items-center gap-3"><CheckCircleIcon class="size-5 shrink-0 text-green-600 dark:text-green-400" /><div><p class="font-medium">{active.name}</p><p class="text-xs text-muted-foreground">{active.url}</p></div></div>
          <Button variant="ghost" size="icon-sm" onclick={deact}><XIcon class="size-4" /></Button>
        </div>
      {:else}
        <div class="text-center text-sm text-muted-foreground"><ArrowRightIcon class="mx-auto mb-1 size-5" /><p class="font-medium">Drop here</p></div>
      {/if}
    </div>

    {#if eps.length > 0}
      <div class="flex flex-wrap gap-1.5">{#each eps as e}<button onclick={() => act(e)} class="rounded-md border px-2 py-0.5 text-xs hover:bg-accent/20 {active?.id === e.id ? 'border-primary bg-accent/30' : ''}" disabled={active?.id === e.id}>{e.name}</button>{/each}</div>
    {/if}

    {#if active}
      {#if active.url.startsWith("http://")}
        <div class="flex items-start gap-2 rounded-lg border border-amber-500/30 bg-amber-50 p-2 text-xs text-amber-700 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-400"><TriangleAlertIcon class="mt-0.5 size-4 shrink-0" /><span>HTTP — enable IP whitelist.</span></div>
      {/if}
      <div class="rounded-lg border bg-card p-4">
        <p class="mb-2 text-sm font-medium"><KeyIcon class="inline size-4" /> Authentication</p>
        {#if authed}
          <div class="mb-2 flex items-center gap-2 text-sm text-green-600 dark:text-green-400"><CheckCircleIcon class="size-4 shrink-0" /> Authenticated</div>
          <Button variant="outline" onclick={logout}><LogOutIcon class="size-4" /> Logout</Button>
        {:else}
          <div class="mb-2"><Input placeholder="username:token" bind:value={key} disabled={testing} /></div>
          <Button onclick={auth} disabled={!key.trim() || testing}>{#if testing}<RefreshCwIcon class="size-4 animate-spin" />{:else}<KeyIcon class="size-4" />{/if} Authenticate</Button>
        {/if}
      </div>
    {/if}
  </div>
</div>
