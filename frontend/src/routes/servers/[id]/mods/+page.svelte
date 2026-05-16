<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { ArrowLeftIcon, PuzzleIcon, RefreshCwIcon, SearchIcon, DownloadIcon, Trash2Icon, PackageIcon, CheckCircleIcon, XCircleIcon, BoxIcon, ExternalLinkIcon, FileIcon, ChevronDownIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Separator } from "$lib/components/ui/separator/index.js";
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import { isAuthenticated, isConfigured, getApi } from "$lib/api";

  let loading = $state(true);
  let serverRunning = $state(false);
  let installed = $state<{ filename: string; name: string; enabled: boolean; size_human: string }[]>([]);
  let searchResults = $state<{ slug: string; title: string; description: string; project_type: string; downloads: number; loaders: string[]; game_versions: string[]; page_url: string; icon_url?: string }[]>([]);
  let searchQuery = $state("");
  let searching = $state(false);
  let tab = $state("installed");
  let filterType = $state("");
  let filterLoader = $state("");
  let filterSort = $state("relevance");
  let mpName = $state(""), mpVer = $state("1.0.0"), generating = $state(false);
  const id = $derived($page.params.id);
  const prov = $derived(($page.params as Record<string, string>).provider ?? "paper");

  const pluginProviders = ["paper", "purpur", "spigot", "waterfall", "velocity"];
  const modProviders = ["fabric", "forge", "neoforge"];

  onMount(async () => {
    if (!isConfigured() || !isAuthenticated()) { goto("/"); return; }
    await fetchInstalled();
  });

  async function fetchInstalled() {
    loading = true;
    try {
      const [mr, sr] = await Promise.all([
        getApi().get<{ items: typeof installed }>(`/api/instances/${id}/mods`).catch(() => ({ items: [] })),
        getApi().get<{ status: Record<string, unknown> }>(`/api/instances/${id}`).catch(() => ({ status: {} })),
      ]);
      installed = mr.items ?? [];
      serverRunning = (sr.status as Record<string, unknown>)?.running === true;
    } catch { /* ignore */ }
    finally { loading = false; }
  }

  function buildSearchUrl() {
    const params = new URLSearchParams({ query: searchQuery, limit: "20" });
    if (filterType) params.set("type", filterType);
    if (filterLoader) params.set("loaders", filterLoader);
    if (filterSort) params.set("index", filterSort);
    return `/api/modrinth/search?${params}`;
  }

  async function search() {
    if (!searchQuery.trim()) return;
    searching = true;
    try {
      const r = await getApi().get<{ results: typeof searchResults }>(buildSearchUrl());
      searchResults = r.results ?? [];
    } catch (e) { toast.error("Search failed", { description: e instanceof Error ? e.message : "" }); }
    finally { searching = false; }
  }

  async function installMod(slug: string) {
    try {
      const info = await getApi().get<{ download_url: string; filename: string }>(`/api/modrinth/project/${slug}/download-url`);
      await getApi().post(`/api/instances/${id}/mods/install`, { download_url: info.download_url, filename: info.filename });
      toast.success(`Installed "${slug}"`);
      fetchInstalled();
    } catch (e) { toast.error("Install failed", { description: e instanceof Error ? e.message : "" }); }
  }

  async function removeMod(filename: string) {
    if (!confirm(`Remove "${filename}"?`)) return;
    try { await getApi().del(`/api/instances/${id}/mods/${filename}`); toast.success("Removed"); fetchInstalled(); }
    catch (e) { toast.error("Remove failed", { description: e instanceof Error ? e.message : "" }); }
  }

  async function toggleMod(filename: string, enabled: boolean) {
    try { await getApi().put(`/api/instances/${id}/mods/${filename}/toggle`, { enabled: !enabled }); fetchInstalled(); }
    catch (e) { toast.error("Toggle failed", { description: e instanceof Error ? e.message : "" }); }
  }

  async function genModpack() {
    if (!mpName.trim()) return;
    generating = true;
    try {
      await getApi().post(`/api/instances/${id}/mods/modpack`, { name: mpName.trim(), version: mpVer, include: installed.filter((i) => i.enabled).map((i) => i.filename) });
      toast.success("Modpack generated! Download from the detail page.");
    } catch (e) { toast.error("Generation failed", { description: e instanceof Error ? e.message : "" }); }
    finally { generating = false; }
  }

  function fmt(n: number): string { return n >= 1_000_000 ? `${(n/1_000_000).toFixed(1)}M` : n >= 1_000 ? `${(n/1_000).toFixed(1)}K` : `${n}`; }

  const typeOpts = [
    { value: "", label: "All types" },
    { value: "mod", label: "Mod" },
    { value: "plugin", label: "Plugin" },
    { value: "datapack", label: "Datapack" },
    { value: "modpack", label: "Modpack" },
    { value: "shader", label: "Shader" },
  ];
  const loaderOpts = [
    { value: "", label: "Any loader" },
    { value: "fabric", label: "Fabric" },
    { value: "forge", label: "Forge" },
    { value: "neoforge", label: "NeoForge" },
    { value: "quilt", label: "Quilt" },
    { value: "paper", label: "Paper" },
    { value: "purpur", label: "Purpur" },
    { value: "spigot", label: "Spigot" },
    { value: "waterfall", label: "Waterfall" },
    { value: "velocity", label: "Velocity" },
  ];
  const sortOpts = [
    { value: "relevance", label: "Relevance" },
    { value: "downloads", label: "Downloads" },
    { value: "follows", label: "Follows" },
    { value: "newest", label: "Newest" },
    { value: "updated", label: "Updated" },
  ];
  const typeLabel = $derived(typeOpts.find((o) => o.value === filterType)?.label ?? "All types");
  const loaderLabel = $derived(loaderOpts.find((o) => o.value === filterLoader)?.label ?? "Any loader");
  const sortLabel = $derived(sortOpts.find((o) => o.value === filterSort)?.label ?? "Relevance");
</script>

<div class="mx-auto max-w-4xl px-6 py-6">
  <button onclick={() => goto(`/servers/${id}`)} class="mb-4 text-sm text-muted-foreground hover:text-foreground"><ArrowLeftIcon class="inline size-4" /> Back</button>
  <div class="mb-4 flex items-center gap-2"><PuzzleIcon class="size-5" /><h1 class="text-xl font-semibold">Mods / Plugins</h1></div>

  {#if loading}
    <div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
  {:else}
    <Tabs.Root bind:value={tab}>
      <Tabs.List class="mb-4">
        <Tabs.Trigger value="installed"><PackageIcon class="size-4" /> Installed ({installed.length})</Tabs.Trigger>
        <Tabs.Trigger value="browse"><SearchIcon class="size-4" /> Browse</Tabs.Trigger>
        <Tabs.Trigger value="modpack"><BoxIcon class="size-4" /> Modpack</Tabs.Trigger>
      </Tabs.List>

      <Tabs.Content value="installed">
        {#if installed.length === 0}
          <Card.Root size="sm"><Card.Content class="py-8 text-center text-sm text-muted-foreground"><PackageIcon class="mx-auto mb-2 size-8" /><p>Nothing installed. Browse Modrinth to find mods/plugins.</p></Card.Content></Card.Root>
        {:else}
          <Card.Root size="sm">
            <div class="overflow-x-auto">
              <table class="w-full text-sm">
                <thead><tr class="border-b text-left text-xs text-muted-foreground"><th class="px-4 py-3 font-medium">Name</th><th class="px-4 py-3 font-medium">File</th><th class="px-4 py-3 font-medium">Size</th><th class="px-4 py-3 font-medium">Status</th><th class="px-4 py-3 font-medium text-right">Actions</th></tr></thead>
                <tbody>
                  {#each installed as m}
                    <tr class="border-b last:border-0 hover:bg-muted/30">
                      <td class="px-4 py-3"><div class="flex items-center gap-2"><FileIcon class="size-4 text-muted-foreground" /><span class="font-medium">{m.name}</span></div></td>
                      <td class="px-4 py-3 font-mono text-xs text-muted-foreground">{m.filename}</td>
                      <td class="px-4 py-3 text-muted-foreground">{m.size_human}</td>
                      <td class="px-4 py-3">{#if m.enabled}<Badge variant="default" class="gap-1 text-xs"><CheckCircleIcon class="size-3" /> Loaded</Badge>{:else}<Badge variant="secondary" class="gap-1 text-xs"><XCircleIcon class="size-3" /> Disabled</Badge>{/if}</td>
                      <td class="px-4 py-3 text-right"><div class="flex items-center justify-end gap-1">
                        <Button variant="ghost" size="icon-xs" onclick={() => toggleMod(m.filename, m.enabled)} title={m.enabled ? "Disable" : "Enable"}>{#if m.enabled}<XCircleIcon class="size-3" />{:else}<CheckCircleIcon class="size-3" />{/if}</Button>
                        <Button variant="ghost" size="icon-xs" class="text-destructive hover:text-destructive" onclick={() => removeMod(m.filename)} title="Remove"><Trash2Icon class="size-3" /></Button>
                      </div></td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          </Card.Root>
        {/if}
      </Tabs.Content>

      <Tabs.Content value="browse">
        <div class="mb-4 flex gap-2">
          <div class="relative flex-1"><SearchIcon class="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" /><Input bind:value={searchQuery} placeholder="Search Modrinth…" onkeydown={(e) => e.key === "Enter" && search()} class="pl-10" /></div>
          <Button onclick={search} disabled={!searchQuery.trim() || searching}>{#if searching}<RefreshCwIcon class="size-4 animate-spin" />{:else}<SearchIcon class="size-4" />{/if} Search</Button>
        </div>
        <!-- Filters -->
        <div class="mb-4 flex flex-wrap items-center gap-2">
          <DropdownMenu.DropdownMenu>
            <DropdownMenu.Trigger>
              <Button variant="outline" size="sm" class="gap-1 text-xs">{typeLabel} <ChevronDownIcon class="size-3" /></Button>
            </DropdownMenu.Trigger>
            <DropdownMenu.Content>
              <DropdownMenu.RadioGroup bind:value={filterType}>
                {#each typeOpts as opt}
                  <DropdownMenu.RadioItem value={opt.value}>{opt.label}</DropdownMenu.RadioItem>
                {/each}
              </DropdownMenu.RadioGroup>
            </DropdownMenu.Content>
          </DropdownMenu.DropdownMenu>
          <DropdownMenu.DropdownMenu>
            <DropdownMenu.Trigger>
              <Button variant="outline" size="sm" class="gap-1 text-xs">{loaderLabel} <ChevronDownIcon class="size-3" /></Button>
            </DropdownMenu.Trigger>
            <DropdownMenu.Content>
              <DropdownMenu.RadioGroup bind:value={filterLoader}>
                {#each loaderOpts as opt}
                  <DropdownMenu.RadioItem value={opt.value}>{opt.label}</DropdownMenu.RadioItem>
                {/each}
              </DropdownMenu.RadioGroup>
            </DropdownMenu.Content>
          </DropdownMenu.DropdownMenu>
          <DropdownMenu.DropdownMenu>
            <DropdownMenu.Trigger>
              <Button variant="outline" size="sm" class="gap-1 text-xs">{sortLabel} <ChevronDownIcon class="size-3" /></Button>
            </DropdownMenu.Trigger>
            <DropdownMenu.Content>
              <DropdownMenu.RadioGroup bind:value={filterSort}>
                {#each sortOpts as opt}
                  <DropdownMenu.RadioItem value={opt.value}>{opt.label}</DropdownMenu.RadioItem>
                {/each}
              </DropdownMenu.RadioGroup>
            </DropdownMenu.Content>
          </DropdownMenu.DropdownMenu>
        </div>
        <div class="max-h-[55dvh] overflow-y-auto pb-6 [mask-image:linear-gradient(to_bottom,black_85%,transparent_100%)]">
        {#if searching}
          <div class="flex items-center justify-center py-12"><RefreshCwIcon class="size-6 animate-spin text-muted-foreground" /></div>
        {:else if searchResults.length > 0}
          <div class="grid gap-3">
            {#each searchResults as r}
              <Card.Root size="sm" class="transition-colors hover:bg-accent/20">
                <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
                <div class="cursor-pointer" onclick={() => goto(`/servers/${id}/mods/${r.slug}`)} onkeydown={(e) => e.key === 'Enter' && goto(`/servers/${id}/mods/${r.slug}`)} role="link" tabindex="0">
                  <Card.Content class="flex items-start justify-between gap-4">
                    <div class="min-w-0 flex-1">
                      <div class="flex items-center gap-2">
                        {#if r.icon_url}
                          <img src={r.icon_url} alt={r.title} class="size-5 shrink-0 rounded object-contain" onerror={(e) => { (e.target as HTMLImageElement).style.display = 'none'; (e.target as HTMLImageElement).nextElementSibling?.classList.remove('hidden'); }} />
                          <PackageIcon class="size-5 shrink-0 text-muted-foreground hidden" />
                        {:else}
                          <PackageIcon class="size-5 shrink-0 text-muted-foreground" />
                        {/if}
                        <span class="font-medium truncate">{r.title}</span>
                        <Badge variant="outline" class="text-[10px] shrink-0">{r.project_type}</Badge>
                      </div>
                      <p class="mt-1 text-xs text-muted-foreground line-clamp-2">{r.description}</p>
                      <div class="mt-2 flex flex-wrap gap-1"><Badge variant="secondary" class="text-[10px]">{fmt(r.downloads)} downloads</Badge>{#each r.loaders.slice(0, 2) as l}<Badge variant="outline" class="text-[10px]">{l}</Badge>{/each}{#each r.game_versions.slice(0, 2) as v}<Badge variant="outline" class="text-[10px]">{v}</Badge>{/each}</div>
                    </div>
                    <div class="flex shrink-0 items-center gap-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="none">
                      <Button size="sm" onclick={() => goto(`/servers/${id}/mods/${r.slug}`)}><DownloadIcon class="size-4" /> Install</Button>
                      <a href={r.page_url} target="_blank" rel="noopener noreferrer"><Button variant="ghost" size="icon-sm"><ExternalLinkIcon class="size-4" /></Button></a>
                    </div>
                  </Card.Content>
                </div>
              </Card.Root>
            {/each}
          </div>
        {:else if searchQuery}
          <Card.Root size="sm"><Card.Content class="py-8 text-center text-sm text-muted-foreground"><SearchIcon class="mx-auto mb-2 size-8" /><p>No results.</p></Card.Content></Card.Root>
        {:else}
          <Card.Root size="sm"><Card.Content class="py-8 text-center text-sm text-muted-foreground"><SearchIcon class="mx-auto mb-2 size-8" /><p>Search for mods and plugins on Modrinth.</p></Card.Content></Card.Root>
        {/if}
        </div>
      </Tabs.Content>

      <Tabs.Content value="modpack">
        <div class="grid gap-4 lg:grid-cols-2">
          <div class="space-y-3">
            <Input placeholder="Modpack name" bind:value={mpName} />
            <div class="grid grid-cols-2 gap-3"><Input placeholder="1.0.0" bind:value={mpVer} /><div class="flex h-9 items-center rounded-md border bg-muted/30 px-2.5 text-sm text-muted-foreground">MC {prov}</div></div>
            <Button onclick={genModpack} disabled={!mpName.trim() || generating} class="w-full">{#if generating}<RefreshCwIcon class="size-4 animate-spin" />{:else}<BoxIcon class="size-4" />{/if} Generate</Button>
          </div>
          <Card.Root size="sm">
            <Card.Content class="py-6 text-center text-sm text-muted-foreground"><BoxIcon class="mx-auto mb-2 size-8" /><p>Generates a .mrpack file from installed mods/plugins. Download from the server detail page.</p></Card.Content>
          </Card.Root>
        </div>
      </Tabs.Content>
    </Tabs.Root>
  {/if}
</div>
