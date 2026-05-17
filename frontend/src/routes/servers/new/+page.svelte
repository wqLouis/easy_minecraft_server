<script lang="ts">
  import { onMount } from "svelte";
  import { toast } from "svelte-sonner";
  import { goto } from "$app/navigation";
  import { ArrowLeftIcon, ServerIcon, RefreshCwIcon, CheckCircleIcon, ChevronRightIcon, ChevronLeftIcon, Settings2Icon, InfoIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import { isAuthenticated, getApi } from "$lib/api";

  let providers = $state<{ name: string; label: string }[]>([]);
  let versions = $state<string[]>([]);
  let selectedProvider = $state(""), selectedVersion = $state("");
  let versionInfo = $state<{ name: string; build: string; java_version: string | null } | null>(null);
  let loadingP = $state(true), loadingV = $state(false), loadingI = $state(false), creating = $state(false);

  let id = $state(""), name = $state("");
  let javaPath = $state("/usr/bin/java"), minMem = $state("1G"), maxMem = $state("4G"), jvmArgs = $state("-XX:+UseG1GC");
  let showAdvanced = $state(false);

  let step = $state(1);
  const totalSteps = 3;

  onMount(() => {
    if (!isAuthenticated()) { toast.error("Authentication required"); goto("/"); return }
    loadProviders();
  });

  async function loadProviders() {
    loadingP = true;
    try { providers = await getApi().get<{ name: string; label: string }[]>("/api/providers") }
    catch (e: unknown) { toast.error("Failed to load providers", { description: e instanceof Error ? e.message : "" }) }
    finally { loadingP = false }
  }

  async function onProviderChange(p: string) {
    selectedProvider = p; selectedVersion = ""; versionInfo = null; versions = [];
    if (!p) return;
    loadingV = true;
    try { versions = (await getApi().get<{ provider: string; versions: string[] }>(`/api/providers/${p}/versions`)).versions }
    catch (e: unknown) { toast.error("Failed to load versions", { description: e instanceof Error ? e.message : "" }) }
    finally { loadingV = false }
  }

  async function onVersionChange(v: string) {
    selectedVersion = v; versionInfo = null;
    if (!v || !selectedProvider) return;
    loadingI = true;
    try {
      const info = await getApi().get<{ name: string; build: string; java_version: string | null }>(`/api/providers/${selectedProvider}/versions/${v}`);
      versionInfo = info;
      const slug = `${selectedProvider}-${v.replace(/\./g, "-")}`;
      id = slug; name = `${providers.find((p) => p.name === selectedProvider)?.label ?? selectedProvider} ${v}`;
    } catch (e: unknown) { toast.error("Failed to fetch version info", { description: e instanceof Error ? e.message : "" }) }
    finally { loadingI = false }
  }

  function canProceedFrom(stepNum: number): boolean {
    if (stepNum === 1) return !!selectedProvider && !!selectedVersion;
    if (stepNum === 2) return !!id.trim() && !!name.trim();
    return true;
  }

  function nextStep() {
    if (!canProceedFrom(step)) return;
    if (step < totalSteps) step++;
  }

  function prevStep() {
    if (step > 1) step--;
  }

  async function handleCreate() {
    if (!id.trim() || !name.trim()) return;
    creating = true;
    try {
      await getApi().post("/api/instances", {
        id: id.trim(), name: name.trim(), provider: selectedProvider, version: selectedVersion,
        java_path: javaPath.trim() || "/usr/bin/java", min_memory: minMem, max_memory: maxMem,
        server_dir: "", jvm_args: jvmArgs.trim() ? jvmArgs.split(/\s+/).filter(Boolean) : undefined,
      });
      toast.success(`Server "${name}" created`);
      goto("/");
    } catch (e: unknown) {
      toast.error("Failed to create server", { description: e instanceof Error ? e.message : "" });
      creating = false;
    }
  }
</script>

<div class="mx-auto max-w-xl px-6 py-6">
  <button onclick={() => goto("/")} class="mb-4 flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground">
    <ArrowLeftIcon class="size-4" /> Back to Servers
  </button>

  <div class="mb-6">
    <h1 class="text-xl font-semibold">New Server</h1>
    <p class="text-sm text-muted-foreground">Choose software and configure your instance.</p>
  </div>

  <!-- Step indicator -->
  <div class="mb-8 flex items-center gap-2">
    {#each { length: totalSteps } as _, i}
      <button onclick={() => { if (i + 1 < step || canProceedFrom(i)) step = i + 1; }}
        class="flex items-center gap-1.5 text-sm {i + 1 === step ? 'text-foreground font-medium' : i + 1 < step ? 'text-primary' : 'text-muted-foreground/50'}">
        <span class="flex size-7 items-center justify-center rounded-full border text-xs font-medium
          {i + 1 === step ? 'border-primary bg-primary/10 text-primary' : i + 1 < step ? 'border-primary bg-primary text-primary-foreground' : 'border-muted-foreground/30'}">
          {i + 1 < step ? '✓' : i + 1}
        </span>
        <span class="hidden sm:inline">
          {#if i === 0}Software{:else if i === 1}Identity{:else}Advanced{/if}
        </span>
        {#if i < totalSteps - 1}<ChevronRightIcon class="size-3 text-muted-foreground/30" />{/if}
      </button>
    {/each}
  </div>

  <Card.Root size="sm">
    <Card.Content class="grid gap-4">
      {#if loadingP}
        <div class="flex items-center justify-center py-8"><RefreshCwIcon class="size-6 animate-spin text-muted-foreground" /></div>
      {:else}
        <!-- Step 1: Software -->
        {#if step === 1}
          <div class="grid gap-2">
            <label for="provider" class="text-sm font-medium">Provider / Server Software</label>
            <select id="provider" value={selectedProvider} onchange={(e) => onProviderChange((e.target as HTMLSelectElement).value)}
              class="dark:bg-input/30 border-input focus-visible:border-ring focus-visible:ring-ring/50 h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm outline-none">
              <option value="">— Select —</option>
              {#each providers as p}<option value={p.name}>{p.label}</option>{/each}
            </select>
          </div>

          {#if selectedProvider}
            <div class="grid gap-2">
              <label for="version" class="text-sm font-medium">Version</label>
              {#if loadingV}
                <div class="flex items-center gap-2 text-sm text-muted-foreground"><RefreshCwIcon class="size-4 animate-spin" /> Loading…</div>
              {:else}
                <select id="version" value={selectedVersion} onchange={(e) => onVersionChange((e.target as HTMLSelectElement).value)}
                  class="dark:bg-input/30 border-input focus-visible:border-ring focus-visible:ring-ring/50 h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm outline-none">
                  <option value="">— Select —</option>
                  {#each versions as v}<option value={v}>{v}</option>{/each}
                </select>
              {/if}
            </div>
          {/if}

          {#if loadingI}
            <div class="flex items-center gap-2 text-sm text-muted-foreground"><RefreshCwIcon class="size-4 animate-spin" /> Fetching info…</div>
          {/if}
          {#if versionInfo}
            <div class="rounded-md border bg-muted/30 p-3 text-sm">
              <div class="flex items-center gap-2 text-green-600 dark:text-green-400"><CheckCircleIcon class="size-4" /> <span class="font-medium">{versionInfo.name}</span></div>
              <div class="mt-1 grid grid-cols-2 gap-1 text-xs text-muted-foreground">
                <span>Build: {versionInfo.build}</span>
                <span>Java: {versionInfo.java_version ?? "auto"}</span>
              </div>
            </div>
          {/if}
        {/if}

        <!-- Step 2: Identity -->
        {#if step === 2}
          <div class="grid gap-2">
            <label for="id" class="text-sm font-medium">Server ID</label>
            <p class="text-xs text-muted-foreground">Used in URLs and file paths. Cannot be changed later.</p>
            <Input id="id" placeholder="my-server" bind:value={id} disabled={creating} />
          </div>
          <div class="grid gap-2">
            <label for="name" class="text-sm font-medium">Server Name</label>
            <p class="text-xs text-muted-foreground">A friendly name to identify your server.</p>
            <Input id="name" placeholder="My Server" bind:value={name} disabled={creating} />
          </div>
        {/if}

        <!-- Step 3: Advanced Config -->
        {#if step === 3}
          <div class="grid gap-2">
            <label for="java" class="text-sm font-medium">Java Binary</label>
            <p class="text-xs text-muted-foreground">Path to the Java executable.</p>
            <Input id="java" placeholder="/usr/bin/java" bind:value={javaPath} disabled={creating} />
          </div>
          <div class="grid grid-cols-2 gap-4">
            <div class="grid gap-2">
              <label for="min" class="text-sm font-medium">Min Memory</label>
              <Input id="min" placeholder="1G" bind:value={minMem} disabled={creating} />
            </div>
            <div class="grid gap-2">
              <label for="max" class="text-sm font-medium">Max Memory</label>
              <Input id="max" placeholder="4G" bind:value={maxMem} disabled={creating} />
            </div>
          </div>
          <div class="grid gap-2">
            <label for="jvm" class="text-sm font-medium">JVM Arguments</label>
            <p class="text-xs text-muted-foreground">Additional JVM flags separated by spaces.</p>
            <Input id="jvm" placeholder="-XX:+UseG1GC" bind:value={jvmArgs} disabled={creating} />
          </div>
        {/if}
      {/if}
    </Card.Content>

    <Card.Footer class="flex justify-between gap-3">
      <div>
        {#if step > 1}
          <Button variant="outline" onclick={prevStep} disabled={creating}>
            <ChevronLeftIcon class="size-4" /> Back
          </Button>
        {:else}
          <span></span>
        {/if}
      </div>
      <div>
        {#if step < totalSteps}
          <Button onclick={nextStep} disabled={!canProceedFrom(step)}>
            Next <ChevronRightIcon class="size-4" />
          </Button>
        {:else}
          <Button onclick={handleCreate} disabled={!selectedProvider || !selectedVersion || !id.trim() || !name.trim() || creating} class="w-full">
            {#if creating}<RefreshCwIcon class="size-4 animate-spin" />{:else}<ServerIcon class="size-4" />{/if}
            {creating ? "Creating…" : "Create Server"}
          </Button>
        {/if}
      </div>
    </Card.Footer>
  </Card.Root>
</div>
