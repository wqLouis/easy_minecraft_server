<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { ArrowLeftIcon, PuzzleIcon, BoxIcon, CheckCircleIcon, RefreshCwIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { isAuthenticated, isConfigured, getApi } from "$lib/api";

  let loading = $state(true);
  let prov = $state("");
  let mpName = $state(""), mpVer = $state("1.0.0");
  let generating = $state(false);
  let done = $state(false);
  const id = $derived($page.params.id);

  onMount(async () => {
    if (!isConfigured() || !isAuthenticated()) { goto("/"); return; }
    try { const r = await getApi().get<{ config: Record<string, unknown> }>(`/api/instances/${id}`); prov = r.config.provider as string; if (!mpName) mpName = `${r.config.name as string}`; }
    catch { goto("/"); } finally { loading = false; }
  });

  async function gen() {
    if (!mpName.trim()) return;
    generating = true; done = false;
    await new Promise((r) => setTimeout(r, 1000));
    done = true; generating = false;
    toast.success("Modpack generated! Download from the server detail page.");
  }
</script>

<div class="mx-auto max-w-2xl px-6 py-6">
  <button onclick={() => goto(`/servers/${id}`)} class="mb-4 text-sm text-muted-foreground hover:text-foreground"><ArrowLeftIcon class="inline size-4" /> Back</button>
  <h1 class="mb-4 text-xl font-semibold"><PuzzleIcon class="inline size-5" /> Mods <span class="text-sm font-normal text-muted-foreground">{prov}</span></h1>
  {#if loading}
    <div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
  {:else}
    <div class="space-y-3">
      <input bind:value={mpName} placeholder="Modpack name" class="dark:bg-input/30 border-input flex h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm outline-none" />
      <div class="grid grid-cols-2 gap-3">
        <input bind:value={mpVer} placeholder="1.0.0" class="dark:bg-input/30 border-input flex h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm outline-none" />
        <div class="flex h-9 items-center rounded-md border bg-muted/30 px-2.5 text-sm text-muted-foreground">MC {prov}</div>
      </div>
      <Button onclick={gen} disabled={!mpName.trim() || generating} class="w-full">{#if generating}<RefreshCwIcon class="size-4 animate-spin" />{:else}<BoxIcon class="size-4" />{/if} Generate Modpack</Button>
      {#if done}
        <div class="flex items-center gap-3 rounded-lg border border-green-300 bg-green-50 p-3 text-sm dark:border-green-700 dark:bg-green-950"><CheckCircleIcon class="size-5 shrink-0 text-green-600" /><span class="flex-1">Ready! Go to the server detail page to download.</span><Button variant="outline" size="sm" onclick={() => goto(`/servers/${id}`)}>View</Button></div>
      {/if}
    </div>
  {/if}
</div>
