<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { ArrowLeftIcon, Settings2Icon, ServerIcon, RefreshCwIcon, SaveIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Separator } from "$lib/components/ui/separator/index.js";
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import { isAuthenticated, isConfigured, getApi } from "$lib/api";

  let cfg = $state<Record<string, unknown>>({});
  let orig = $state("");
  let props = $state<Record<string, string>>({});
  let origProps = $state("");
  let loading = $state(true);
  let saving = $state(false);
  let tab = $state("instance");
  const id = $derived($page.params.id);
  const chg = $derived(JSON.stringify(cfg) !== orig);
  const chgProps = $derived(JSON.stringify(props) !== origProps);

  onMount(async () => {
    if (!isConfigured() || !isAuthenticated()) { goto("/"); return; }
    try {
      const r = await getApi().get<{ config: Record<string, unknown> }>(`/api/instances/${id}`);
      cfg = { ...r.config }; orig = JSON.stringify(r.config);
      const p = await getApi().get<{ properties: Record<string, string> }>(`/api/instances/${id}/properties`).catch(() => ({ properties: {} }));
      props = p.properties ?? {}; origProps = JSON.stringify(props);
    } catch { goto("/"); } finally { loading = false; }
  });

  function set(k: string, v: unknown) { cfg = { ...cfg, [k]: v }; }
  function setProp(k: string, v: string) { props = { ...props, [k]: v }; }

  async function saveCfg() {
    saving = true;
    try { await getApi().put(`/api/instances/${id}/config`, cfg); orig = JSON.stringify(cfg); toast.success("Config saved"); }
    catch (e) { toast.error("Failed", { description: e instanceof Error ? e.message : "" }); }
    saving = false;
  }

  async function saveProps() {
    saving = true;
    try { await getApi().put(`/api/instances/${id}/properties`, props); origProps = JSON.stringify(props); toast.success("Properties saved"); }
    catch (e) { toast.error("Failed", { description: e instanceof Error ? e.message : "" }); }
    saving = false;
  }
</script>

<div class="mx-auto max-w-2xl px-6 py-6">
  <button onclick={() => goto(`/servers/${id}`)} class="mb-4 text-sm text-muted-foreground hover:text-foreground"><ArrowLeftIcon class="inline size-4" /> Back</button>
  <div class="mb-4 flex items-center gap-2"><Settings2Icon class="size-5" /><h1 class="flex-1 text-xl font-semibold">Configuration</h1></div>

  {#if loading}
    <div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
  {:else}
    <Tabs.Root bind:value={tab}>
      <Tabs.List class="mb-4">
        <Tabs.Trigger value="instance"><ServerIcon class="size-4" /> Instance</Tabs.Trigger>
        <Tabs.Trigger value="properties"><Settings2Icon class="size-4" /> server.properties</Tabs.Trigger>
      </Tabs.List>

      <Tabs.Content value="instance">
        <div class="space-y-3">
          <label class="text-sm font-medium">Name<input value={cfg.name as string ?? ""} oninput={(e) => set("name", (e.target as HTMLInputElement).value)} class="dark:bg-input/30 border-input mt-1 flex h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm outline-none" /></label>
          <div class="grid grid-cols-2 gap-3">
            <label class="text-sm font-medium">Min Memory<input value={cfg.min_memory as string ?? "1G"} oninput={(e) => set("min_memory", (e.target as HTMLInputElement).value)} class="dark:bg-input/30 border-input mt-1 flex h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm outline-none" /></label>
            <label class="text-sm font-medium">Max Memory<input value={cfg.max_memory as string ?? "4G"} oninput={(e) => set("max_memory", (e.target as HTMLInputElement).value)} class="dark:bg-input/30 border-input mt-1 flex h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm outline-none" /></label>
          </div>
          <label class="text-sm font-medium">Java Path<input value={cfg.java_path as string ?? ""} oninput={(e) => set("java_path", (e.target as HTMLInputElement).value)} class="dark:bg-input/30 border-input mt-1 flex h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm outline-none" /></label>
          <label class="text-sm font-medium">JVM Args<input value={(cfg.jvm_args as string[])?.join(" ") ?? ""} oninput={(e) => set("jvm_args", (e.target as HTMLInputElement).value.split(/\s+/).filter(Boolean))} class="dark:bg-input/30 border-input mt-1 flex h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm outline-none" /></label>
          <Button onclick={saveCfg} disabled={!chg || saving} class="w-full mt-2">{#if saving}<RefreshCwIcon class="size-4 animate-spin" />{:else}<SaveIcon class="size-4" />{/if} Save Instance Config</Button>
        </div>
      </Tabs.Content>

      <Tabs.Content value="properties">
        {#if Object.keys(props).length === 0}
          <div class="rounded-lg border bg-card p-8 text-center text-sm text-muted-foreground"><Settings2Icon class="mx-auto mb-2 size-8" /><p>No server.properties found.</p></div>
        {:else}
          <div class="space-y-3">
            {#each Object.entries(props) as [key, val]}
              <label class="text-sm font-medium">{key}<input value={val} oninput={(e) => setProp(key, (e.target as HTMLInputElement).value)} class="dark:bg-input/30 border-input mt-1 flex h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm font-mono outline-none" /></label>
            {/each}
            <Button onclick={saveProps} disabled={!chgProps || saving} class="w-full mt-2">{#if saving}<RefreshCwIcon class="size-4 animate-spin" />{:else}<SaveIcon class="size-4" />{/if} Save Properties</Button>
          </div>
        {/if}
      </Tabs.Content>
    </Tabs.Root>
  {/if}
</div>
