<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { ArrowLeftIcon, PuzzleIcon, RefreshCwIcon, DownloadIcon, PackageIcon, ExternalLinkIcon, GlobeIcon, UsersIcon, ChevronDownIcon, CheckCircleIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import { isAuthenticated, isConfigured, getApi } from "$lib/api";

  let project = $state<Record<string, unknown> | null>(null);
  let versions = $state<{ id: string; name: string; version_number: string; loaders: string[]; game_versions: string[] }[]>([]);
  let filteredVersions = $state<typeof versions>([]);
  let loading = $state(true);
  let installing = $state(false);
  let selectedMcVer = $state("");
  let selectedLoader = $state("");

  const id = $derived($page.params.id);
  const slug = $derived($page.params.slug);
  const prov = $derived(($page.params as Record<string, string>).provider ?? "");

  function getLoaderHints(): Set<string> {
    if (prov === "fabric" || prov === "forge" || prov === "neofabric" || prov === "quilt") return new Set(["fabric", "forge", "neoforge", "quilt"]);
    if (["paper", "purpur", "spigot", "waterfall", "velocity"].includes(prov)) return new Set(["paper", "purpur", "spigot", "waterfall", "velocity"]);
    return new Set();
  }
  const loaderHints = $derived(getLoaderHints());

  onMount(async () => {
    if (!isConfigured() || !isAuthenticated()) { goto("/"); return; }
    await Promise.all([fetchProject(), fetchVersions()]);
  });

  $effect(() => {
    filterVersions();
  });

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

  function filterVersions() {
    let v = versions;
    if (selectedMcVer) v = v.filter((ver) => ver.game_versions.includes(selectedMcVer));
    if (selectedLoader) v = v.filter((ver) => ver.loaders.includes(selectedLoader));
    filteredVersions = v;
  }

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

  async function installSelected() {
    if (!selectedMcVer || !selectedLoader) { toast.error("Select a Minecraft version and loader"); return; }
    installing = true;
    try {
      const info = await getApi().get<{ download_url: string; filename: string }>(
        `/api/modrinth/project/${slug}/download-url?mc_version=${encodeURIComponent(selectedMcVer)}&loader=${encodeURIComponent(selectedLoader)}`
      );
      await getApi().post(`/api/instances/${id}/mods/install`, { download_url: info.download_url, filename: info.filename });
      toast.success(`Installed "${project?.title ?? slug}"`);
      goto(`/servers/${id}/mods`);
    } catch (e) { toast.error("Install failed", { description: e instanceof Error ? e.message : "" }); }
    finally { installing = false; }
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

    <!-- Version selection -->
    <Card.Root size="sm" class="mb-6">
      <Card.Content class="p-4">
        <h2 class="mb-3 text-sm font-semibold">Install a version</h2>
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

        <!-- Version list -->
        <div class="max-h-64 overflow-y-auto rounded-lg border" style="scrollbar-color: hsl(var(--border)) transparent; scrollbar-width: thin;">
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
                <Button size="sm" onclick={async () => {
                  selectedMcVer = ver.game_versions[0] || selectedMcVer;
                  selectedLoader = ver.loaders[0] || selectedLoader;
                  await installSelected();
                }} disabled={installing} class="shrink-0 ml-3">
                  <DownloadIcon class="size-4" /> Install
                </Button>
              </div>
            {/each}
          {/if}
        </div>
      </Card.Content>
    </Card.Root>

    <!-- Bulk install button -->
    <div class="flex justify-end">
      <Button onclick={installSelected} disabled={!selectedMcVer || !selectedLoader || installing} class="gap-2">
        {#if installing}<RefreshCwIcon class="size-4 animate-spin" />{/if}
        <DownloadIcon class="size-4" /> Install with selected options
      </Button>
    </div>
  {/if}
</div>
