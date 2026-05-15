<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { ArrowLeftIcon, Settings2Icon, RefreshCwIcon, SaveIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { isAuthenticated, isConfigured, getApi } from "$lib/api";

  let cfg = $state<Record<string, unknown>>({});
  let orig = $state("");
  let loading = $state(true);
  let saving = $state(false);
  const id = $derived($page.params.id);
  const chg = $derived(JSON.stringify(cfg) !== orig);
  let nameEl: HTMLInputElement, minEl: HTMLInputElement, maxEl: HTMLInputElement, javaEl: HTMLInputElement, jvmEl: HTMLInputElement;

  onMount(async () => {
    if (!isConfigured() || !isAuthenticated()) { goto("/"); return; }
    try { const r = await getApi().get<{ config: Record<string, unknown> }>(`/api/instances/${id}`); cfg = { ...r.config }; orig = JSON.stringify(r.config); }
    catch { goto("/"); } finally { loading = false; }
  });

  function read() { cfg = { ...cfg, name: nameEl.value, min_memory: minEl.value, max_memory: maxEl.value, java_path: javaEl.value, jvm_args: jvmEl.value.split(/\s+/).filter(Boolean) }; }

  async function save() {
    saving = true; read();
    try { await getApi().put(`/api/instances/${id}/config`, cfg); orig = JSON.stringify(cfg); toast.success("Saved"); }
    catch (e) { toast.error("Failed", { description: e instanceof Error ? e.message : "" }); }
    saving = false;
  }
</script>

<div class="mx-auto max-w-2xl px-6 py-6">
  <button onclick={() => goto(`/servers/${id}`)} class="mb-4 text-sm text-muted-foreground hover:text-foreground"><ArrowLeftIcon class="inline size-4" /> Back</button>
  <div class="mb-4 flex items-center gap-2">
    <Settings2Icon class="size-5" />
    <h1 class="flex-1 text-xl font-semibold">Configuration</h1>
    <Button variant="outline" size="sm" onclick={() => location.reload()} disabled={loading}><RefreshCwIcon class={loading ? "size-4 animate-spin" : "size-4"} /></Button>
  </div>
  {#if loading}
    <div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
  {:else}
    <div class="space-y-3">
      <label class="text-sm font-medium">Name<input bind:this={nameEl} value={cfg.name as string ?? ""} oninput={read} class="dark:bg-input/30 border-input mt-1 flex h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm outline-none" /></label>
      <div class="grid grid-cols-2 gap-3">
        <label class="text-sm font-medium">Min Memory<input bind:this={minEl} value={cfg.min_memory as string ?? "1G"} oninput={read} class="dark:bg-input/30 border-input mt-1 flex h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm outline-none" /></label>
        <label class="text-sm font-medium">Max Memory<input bind:this={maxEl} value={cfg.max_memory as string ?? "4G"} oninput={read} class="dark:bg-input/30 border-input mt-1 flex h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm outline-none" /></label>
      </div>
      <label class="text-sm font-medium">Java Path<input bind:this={javaEl} value={cfg.java_path as string ?? ""} oninput={read} class="dark:bg-input/30 border-input mt-1 flex h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm outline-none" /></label>
      <label class="text-sm font-medium">JVM Args<input bind:this={jvmEl} value={(cfg.jvm_args as string[])?.join(" ") ?? ""} oninput={read} class="dark:bg-input/30 border-input mt-1 flex h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm outline-none" /></label>
      <Button onclick={save} disabled={!chg || saving} class="w-full">{#if saving}<RefreshCwIcon class="size-4 animate-spin" />{:else}<SaveIcon class="size-4" />{/if} Save</Button>
    </div>
  {/if}
</div>
