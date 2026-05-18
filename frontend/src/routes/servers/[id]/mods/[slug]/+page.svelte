<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { ArrowLeftIcon, PuzzleIcon, RefreshCwIcon, DownloadIcon, PackageIcon, ExternalLinkIcon, GlobeIcon, UsersIcon, ChevronDownIcon, CheckCircleIcon, Link2Icon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import { isAuthenticated, isConfigured, getApi } from "$lib/api";

  let project = $state<Record<string, unknown> | null>(null);
  let versions = $state<{ id: string; name: string; version_number: string; loaders: string[]; game_versions: string[] }[]>([]);
  let filteredVersions = $state<typeof versions>([]);
  let dependencies = $state<{ slug: string; title: string; download_url?: string; filename?: string; icon_url?: string }[]>([]);
  let loading = $state(true);
  let installing = $state(false);
  let selectedMcVer = $state("");
  let selectedLoader = $state("");
  let installingVersion = $state<string | null>(null);
  let provider = $state("");
  let mcVersion = $state("");

  const id = $derived($page.params.id);
  const slug = $derived($page.params.slug);

  function getLoaderHints(): Set<string> {
    if (provider === "fabric" || provider === "forge" || provider === "neofabric" || provider === "quilt") return new Set(["fabric", "forge", "neoforge", "quilt"]);
    if (["paper", "purpur", "spigot", "waterfall", "velocity"].includes(provider)) return new Set(["paper", "purpur", "spigot", "waterfall", "velocity"]);
    return new Set();
  }
  const loaderHints = $derived(getLoaderHints());

  onMount(async () => {
    if (!isConfigured() || !isAuthenticated()) { goto("/"); return; }
    await Promise.all([fetchProject(), fetchVersions(), fetchProvider()]);
  });

  async function fetchProvider() {
    try {
      const r = await getApi().get<{ config: Record<string, unknown> }>(`/api/instances/${id}`);
      const cfg = r.config as Record<string, unknown>;
      provider = (cfg.provider as string) ?? "";
      mcVersion = (cfg.version as string) ?? "";
    } catch { /* ignore */ }
  }

  async function fetchProject() {
    try {
      const r = await getApi().get<Record<string, unknown>>(`/api/modrinth/project/${slug}`);
      project = r;
    } catch { toast.error("Failed to load project"); goto(`/servers/${id}/mods`); }
  }

  async function fetchVersions() {
    try {
      const r = await getApi().get<{ versions: typeof versions }>(`/api/modrinth/project/${slug}/versions`);
      versions = r.versions ?? [];
    } catch { toast.error("Failed to load versions"); }
    finally { loading = false; }
  }

  // Auto-select server's MC version and loader once versions are loaded
  $effect(() => {
    if (!versions.length || !provider || !mcVersion) return;
    if (!selectedMcVer) {
      if (uniqueMcVersions().includes(mcVersion)) {
        selectedMcVer = mcVersion;
      } else {
        selectedMcVer = uniqueMcVersions()[0] ?? "";
      }
    }
    if (!selectedLoader) {
      const preferred = uniqueLoaders();
      selectedLoader = preferred[0] ?? "";
    }
  });

  // Fetch dependencies whenever selection changes (only for MC versions the mod actually supports)
  $effect(() => {
    if (!selectedMcVer || !selectedLoader) return;
    // Skip initial auto-select bootstrap — only fetch when user-changed or stable
    if (!uniqueMcVersions().includes(selectedMcVer)) return;
    fetchDependencies();
  });

  async function fetchDependencies() {
    try {
      const deps = await getApi().get<{ slug: string; title: string; download_url?: string; filename?: string; icon_url?: string }[]>(
        `/api/modrinth/project/${slug}/dependencies?mc_version=${encodeURIComponent(selectedMcVer)}&loader=${encodeURIComponent(selectedLoader)}`
      );
      dependencies = deps ?? [];
    } catch { dependencies = []; }
  }

  function filterVersions() {
    let v = versions;
    if (selectedMcVer) v = v.filter((ver) => ver.game_versions.includes(selectedMcVer));
    if (selectedLoader) v = v.filter((ver) => ver.loaders.includes(selectedLoader));
    filteredVersions = v;
  }

  $effect(() => { filterVersions(); });

  function uniqueMcVersions(): string[] {
    const set = new Set<string>();
    for (const v of versions) for (const gv of v.game_versions) set.add(gv);
    return [...set].sort().reverse();
  }

  function uniqueLoaders(): string[] {
    const set = new Set<string>();
    for (const v of versions) for (const l of v.loaders) set.add(l);
    const preferred = [...loaderHints].filter((l) => set.has(l));
    const rest = [...set].filter((l) => !loaderHints.has(l));
    return [...preferred, ...rest];
  }

  async function installVersion(ver: typeof filteredVersions[number]) {
    installingVersion = ver.id;
    try {
      const verMc = ver.game_versions[0] || selectedMcVer;
      const verLoader = ver.loaders[0] || selectedLoader;
      // Resolve and install dependencies for this specific version
      const deps = await getApi().get<{ slug: string; title: string; download_url?: string; filename?: string }[]>(
        `/api/modrinth/project/${slug}/dependencies?mc_version=${encodeURIComponent(verMc)}&loader=${encodeURIComponent(verLoader)}`
      ).catch(() => []);
      dependencies = deps; // update display for the user
      for (const dep of deps) {
        if (!dep.download_url || !dep.filename) continue;
        try {
          await getApi().post(`/api/instances/${id}/mods/install`, { download_url: dep.download_url, filename: dep.filename });
        } catch { /* skip if already installed */ }
      }
      const info = await getApi().get<{ download_url: string; filename: string }>(
        `/api/modrinth/project/${slug}/download-url?mc_version=${encodeURIComponent(verMc)}&loader=${encodeURIComponent(verLoader)}`
      );
      await getApi().post(`/api/instances/${id}/mods/install`, { download_url: info.download_url, filename: info.filename });
      toast.success(`Installed "${project?.title ?? slug}"`);
      goto(`/servers/${id}/mods`);
    } catch (e) { toast.error("Install failed", { description: e instanceof Error ? e.message : "" }); }
    finally { installingVersion = null; }
  }

  function fmt(n: number): string { return n >= 1_000_000 ? `${(n / 1_000_000).toFixed(1)}M` : n >= 1_000 ? `${(n / 1_000).toFixed(1)}K` : `${n}`; }
</script>

<div class="mx-auto max-w-4xl px-6 py-6">
  <button onclick={() => goto(`/servers/${id}/mods`)} class="mb-4 text-sm text-muted-foreground hover:text-foreground"><ArrowLeftIcon class="inline size-4" /> Back to Mods</button>

  {#if loading || !project}
    <div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
  {:else}
    <!-- Project header -->
    <div class="mb-6 flex items-start gap-4">
      {#if (project.icon_url as string)}
        <img src={project.icon_url as string} alt={project.title as string} class="size-14 shrink-0 rounded-xl object-contain" />
      {:else}
        <div class="flex size-14 shrink-0 items-center justify-center rounded-xl bg-muted"><PackageIcon class="size-7 text-muted-foreground" /></div>
      {/if}
      <div class="min-w-0 flex-1">
        <div class="flex items-center gap-2">
          <h1 class="text-xl font-semibold">{project.title as string}</h1>
          <Badge variant="outline" class="text-xs">{project.project_type as string}</Badge>
        </div>
        <p class="mt-1 text-sm text-muted-foreground">{project.description as string}</p>
        <div class="mt-3 flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
          <span class="flex items-center gap-1"><DownloadIcon class="size-3.5" /> {fmt(project.downloads as number)} downloads</span>
          <span class="flex items-center gap-1"><UsersIcon class="size-3.5" /> {fmt(project.follows as number)} follows</span>
          <span>Client: {project.client_side as string ?? "unknown"}</span>
          <span>Server: {project.server_side as string ?? "unknown"}</span>
          <a href={project.page_url as string} target="_blank" rel="noopener noreferrer" class="flex items-center gap-1 text-primary hover:underline"><ExternalLinkIcon class="size-3.5" /> Modrinth</a>
        </div>
      </div>
    </div>

    <!-- Version selection (no longer wrapped in a Card) -->
    <h2 class="mb-3 text-sm font-semibold">Versions</h2>
    <div class="mb-4 flex flex-wrap gap-3">
      <DropdownMenu.DropdownMenu>
        <DropdownMenu.Trigger>
          <Button variant="outline" size="sm" class="gap-1 text-xs">{selectedMcVer || "MC version"} <ChevronDownIcon class="size-3" /></Button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Content>
          <DropdownMenu.RadioGroup bind:value={selectedMcVer}>
            <DropdownMenu.RadioItem value="">Any</DropdownMenu.RadioItem>
            {#each uniqueMcVersions() as mv}
              <DropdownMenu.RadioItem value={mv}>{mv}</DropdownMenu.RadioItem>
            {/each}
          </DropdownMenu.RadioGroup>
        </DropdownMenu.Content>
      </DropdownMenu.DropdownMenu>
      <DropdownMenu.DropdownMenu>
        <DropdownMenu.Trigger>
          <Button variant="outline" size="sm" class="gap-1 text-xs">{selectedLoader || "Loader"} <ChevronDownIcon class="size-3" /></Button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Content>
          <DropdownMenu.RadioGroup bind:value={selectedLoader}>
            <DropdownMenu.RadioItem value="">Any</DropdownMenu.RadioItem>
            {#each uniqueLoaders() as l}
              <DropdownMenu.RadioItem value={l}>{l}</DropdownMenu.RadioItem>
            {/each}
          </DropdownMenu.RadioGroup>
        </DropdownMenu.Content>
      </DropdownMenu.DropdownMenu>
    </div>

    <!-- Version list (no card wrapper) -->
    <div class="mb-6 rounded-lg border" style="scrollbar-color: hsl(var(--border)) transparent; scrollbar-width: thin;">
      {#if filteredVersions.length === 0}
        <p class="py-8 text-center text-sm text-muted-foreground">No versions match the selected filters.</p>
      {:else}
        {#each filteredVersions as ver}
          <div class="flex items-center justify-between border-b last:border-0 px-4 py-3 hover:bg-muted/20">
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2">
                <span class="text-sm font-medium">{ver.name}</span>
                <Badge variant="secondary" class="text-[10px]">{ver.version_number}</Badge>
              </div>
              <div class="mt-1 flex flex-wrap gap-1">
                {#each ver.loaders as l}<Badge variant="outline" class="text-[10px]">{l}</Badge>{/each}
                {#each ver.game_versions as gv}<Badge variant="outline" class="text-[10px]">{gv}</Badge>{/each}
              </div>
            </div>
            <Button size="sm" onclick={() => installVersion(ver)} disabled={installingVersion !== null} class="shrink-0 ml-3">
              {#if installingVersion === ver.id}
                <RefreshCwIcon class="size-4 animate-spin" />
              {:else}
                <DownloadIcon class="size-4" />
              {/if}
              Install
            </Button>
          </div>
        {/each}
      {/if}
    </div>

    <!-- Dependencies section -->
    {#if dependencies.length > 0}
      <h2 class="mb-3 text-sm font-semibold">Required Dependencies</h2>
      <div class="rounded-lg border">
        {#each dependencies as dep}
          <div class="flex items-center gap-3 border-b last:border-0 px-4 py-3">
            {#if dep.icon_url}
              <img src={dep.icon_url} alt={dep.title} class="size-8 shrink-0 rounded object-contain" />
            {:else}
              <div class="flex size-8 shrink-0 items-center justify-center rounded bg-muted"><Link2Icon class="size-4 text-muted-foreground" /></div>
            {/if}
            <div class="min-w-0 flex-1">
              <span class="text-sm font-medium">{dep.title}</span>
              <span class="ml-2 text-[10px] text-muted-foreground">{dep.slug}</span>
            </div>
            {#if dep.download_url}
              <a href={dep.download_url} target="_blank" rel="noopener noreferrer" class="shrink-0">
                <Button variant="outline" size="sm" class="gap-1 text-xs"><ExternalLinkIcon class="size-3" /> Modrinth</Button>
              </a>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>
