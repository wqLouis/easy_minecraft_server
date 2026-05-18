<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { toast } from 'svelte-sonner';
	import {
		ArrowLeftIcon,
		PuzzleIcon,
		RefreshCwIcon,
		SearchIcon,
		DownloadIcon,
		Trash2Icon,
		PackageIcon,
		CheckCircleIcon,
		XCircleIcon,
		BoxIcon,
		ExternalLinkIcon,
		FileIcon,
		ChevronDownIcon,
		PlusIcon,
		ListIcon,
		XIcon,
		LoaderCircleIcon
	} from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import * as Card from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import * as Tabs from '$lib/components/ui/tabs/index.js';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js';
	import { isAuthenticated, isConfigured, getApi } from '$lib/api';

	let loading = $state(true);
	let serverRunning = $state(false);
	let installed = $state<
		{ filename: string; name: string; enabled: boolean; size_human: string }[]
	>([]);
	let searchResults = $state<
		{
			slug: string;
			title: string;
			description: string;
			project_type: string;
			downloads: number;
			loaders: string[];
			game_versions: string[];
			page_url: string;
			icon_url?: string;
		}[]
	>([]);
	let searchQuery = $state('');
	let searching = $state(false);
	let loadingMore = $state(false);
	let hasMore = $state(true);
	const PAGE_SIZE = 24;
	let tab = $state('installed');
	let filterType = $state('');
	let filterLoader = $state('');
	let filterVersion = $state('');
	let filterSort = $state('relevance');
	let mpName = $state($page.params.id ?? ''),
		mpVer = $state('1.0.0'),
		generating = $state(false);

	// ── Install Queue ──────────────────────────────────────────
	interface QueueItem {
		slug: string;
		title: string;
		icon_url?: string;
	}
	let installQueue = $state<QueueItem[]>([]);
	let isInstallingAll = $state(false);
	let installProgress = $state({ current: 0, total: 0 });
	let installErrors = $state<string[]>([]);

	let installingMod = $state<string | null>(null);
	const id = $derived($page.params.id);
	let provider = $state('');
	let mcVersion = $state('');

	// Derive default type and loader from the server provider
	const providerDefaults = $derived.by(() => {
		const p = provider.toLowerCase();
		if (['fabric', 'forge', 'neoforge', 'quilt'].includes(p)) return { type: 'mod', loader: p };
		if (['paper', 'purpur', 'spigot', 'waterfall', 'velocity'].includes(p))
			return { type: 'plugin', loader: p };
		if (p === 'vanilla') return { type: 'datapack', loader: '' };
		return { type: '', loader: '' };
	});

	// Initialise filters once provider is known
	$effect(() => {
		if (provider && !filterType && !filterLoader && !filterVersion) {
			filterType = providerDefaults.type;
			filterLoader = providerDefaults.loader;
			if (mcVersion) filterVersion = mcVersion;
		}
	});

	onMount(async () => {
		if (!isConfigured() || !isAuthenticated()) {
			goto('/');
			return;
		}
		await fetchInstalled();
	});

	/// Full initial load: installed mods + server metadata + popular mods.
	async function fetchInstalled() {
		loading = true;
		try {
			const [mr, inst] = await Promise.all([
				getApi()
					.get<{ items: typeof installed }>(`/api/instances/${id}/mods`)
					.catch(() => ({ items: [] })),
				getApi()
					.get<{ config: Record<string, unknown>; status: Record<string, unknown> }>(
						`/api/instances/${id}`
					)
					.catch(() => ({ config: {}, status: {} }))
			]);
			installed = mr.items ?? [];
			serverRunning = (inst.status as Record<string, unknown>)?.running === true;
			const cfg = inst.config as Record<string, unknown>;
			provider = (cfg.provider as string) ?? '';
			mcVersion = (cfg.version as string) ?? '';
			loadPopular();
		} catch {
			/* ignore */
		} finally {
			loading = false;
		}
	}

	async function refreshInstalled() {
		try {
			const mr = await getApi()
				.get<{ items: typeof installed }>(`/api/instances/${id}/mods`)
				.catch(() => ({ items: [] }));
			installed = mr.items ?? [];
		} catch {
			/* ignore */
		}
	}

	function buildSearchUrl(query: string, off: number = 0) {
		const params = new URLSearchParams({ query, limit: String(PAGE_SIZE), offset: String(off) });
		if (filterType) params.set('type', filterType);
		if (filterLoader) params.set('loaders', filterLoader);
		if (filterVersion) params.set('versions', filterVersion);
		if (filterSort) params.set('index', filterSort);
		return `/api/modrinth/search?${params}`;
	}

	async function loadPopular() {
		searching = true;
		hasMore = true;
		try {
			const r = await getApi().get<{ results: typeof searchResults }>(buildSearchUrl('', 0));
			searchResults = r.results ?? [];
			hasMore = (r.results?.length ?? 0) >= PAGE_SIZE;
		} catch {
			/* ignore — user can search manually */
		} finally {
			searching = false;
		}
	}

	async function search(reset: boolean = true) {
		if (reset) {
			searching = true;
			hasMore = true;
		} else {
			loadingMore = true;
		}
		const off = reset ? 0 : searchResults.length;
		try {
			const r = await getApi().get<{ results: typeof searchResults }>(
				buildSearchUrl(searchQuery, off)
			);
			if (reset) {
				searchResults = r.results ?? [];
			} else {
				searchResults = [...searchResults, ...(r.results ?? [])];
			}
			hasMore = (r.results?.length ?? 0) >= PAGE_SIZE;
		} catch (e) {
			toast.error('Search failed', { description: e instanceof Error ? e.message : '' });
		} finally {
			searching = false;
			loadingMore = false;
		}
	}

	// ── Queue Operations ────────────────────────────────────────

	function isInQueue(slug: string) {
		return installQueue.some((q) => q.slug === slug);
	}

	function toggleQueue(r: QueueItem) {
		if (isInQueue(r.slug)) {
			installQueue = installQueue.filter((q) => q.slug !== r.slug);
		} else {
			installQueue = [...installQueue, r];
		}
	}

	function clearQueue() {
		installQueue = [];
		installErrors = [];
	}

	// ── Batch Install ──────────────────────────────────────────

	async function installAll() {
		if (installQueue.length === 0) return;
		isInstallingAll = true;
		installErrors = [];
		installProgress = { current: 0, total: installQueue.length };

		const loader = providerDefaults.loader;
		const ver = mcVersion;
		if (!loader || !ver) {
			toast.error('Server provider or version not loaded yet');
			isInstallingAll = false;
			return;
		}

		// Process one by one
		for (const item of installQueue) {
			installProgress = { ...installProgress, current: installProgress.current + 1 };
			try {
				// Resolve and install dependencies first
				const deps = await getApi()
					.get<
						{
							slug: string;
							title: string;
							download_url?: string;
							filename?: string;
							icon_url?: string;
						}[]
					>(
						`/api/modrinth/project/${item.slug}/dependencies?mc_version=${encodeURIComponent(ver)}&loader=${encodeURIComponent(loader)}`
					)
					.catch(() => []);
				for (const dep of deps) {
					if (!dep.download_url || !dep.filename) continue;
					try {
						await getApi().post(`/api/instances/${id}/mods/install`, {
							download_url: dep.download_url,
							filename: dep.filename
						});
					} catch {
						/* skip if already installed */
					}
				}
				// Install the main mod
				const info = await getApi().get<{ download_url: string; filename: string }>(
					`/api/modrinth/project/${item.slug}/download-url?mc_version=${encodeURIComponent(ver)}&loader=${encodeURIComponent(loader)}`
				);
				await getApi().post(`/api/instances/${id}/mods/install`, {
					download_url: info.download_url,
					filename: info.filename
				});
			} catch (e) {
				const msg = e instanceof Error ? e.message : 'Unknown error';
				installErrors = [...installErrors, `${item.title}: ${msg}`];
			}
		}

		isInstallingAll = false;
		if (installErrors.length === 0) {
			toast.success(`Installed ${installQueue.length} mod(s)`);
		} else {
			toast.error(`${installErrors.length} installation(s) failed`);
		}
		clearQueue();
		refreshInstalled();
	}

	// ── Single Mod Install (for dep install, keep for now) ────

	async function installMod(slug: string) {
		installingMod = slug;
		try {
			const loader = providerDefaults.loader;
			const ver = mcVersion;
			if (!loader || !ver) {
				toast.error('Server provider or version not loaded yet');
				installingMod = null;
				return;
			}
			const deps = await getApi()
				.get<
					{
						slug: string;
						title: string;
						download_url?: string;
						filename?: string;
						icon_url?: string;
					}[]
				>(
					`/api/modrinth/project/${slug}/dependencies?mc_version=${encodeURIComponent(ver)}&loader=${encodeURIComponent(loader)}`
				)
				.catch(() => []);
			for (const dep of deps) {
				if (!dep.download_url || !dep.filename) continue;
				try {
					await getApi().post(`/api/instances/${id}/mods/install`, {
						download_url: dep.download_url,
						filename: dep.filename
					});
					toast.success(`Dependency installed: ${dep.title}`);
				} catch {
					/* skip if already installed or fails */
				}
			}
			const info = await getApi().get<{ download_url: string; filename: string }>(
				`/api/modrinth/project/${slug}/download-url?mc_version=${encodeURIComponent(ver)}&loader=${encodeURIComponent(loader)}`
			);
			await getApi().post(`/api/instances/${id}/mods/install`, {
				download_url: info.download_url,
				filename: info.filename
			});
			toast.success(`Installed "${slug}"`);
			refreshInstalled();
		} catch (e) {
			toast.error('Install failed', { description: e instanceof Error ? e.message : '' });
		} finally {
			installingMod = null;
		}
	}

	async function removeMod(filename: string) {
		if (!confirm(`Remove "${filename}"?`)) return;
		try {
			await getApi().del(`/api/instances/${id}/mods/${filename}`);
			toast.success('Removed');
			refreshInstalled();
		} catch (e) {
			toast.error('Remove failed', { description: e instanceof Error ? e.message : '' });
		}
	}

	async function toggleMod(filename: string, enabled: boolean) {
		try {
			await getApi().put(`/api/instances/${id}/mods/${filename}/toggle`, { enabled: !enabled });
			refreshInstalled();
		} catch (e) {
			toast.error('Toggle failed', { description: e instanceof Error ? e.message : '' });
		}
	}

	async function genModpack() {
		if (!mpName.trim()) return;
		generating = true;
		try {
			await getApi().post(`/api/instances/${id}/mods/modpack`, {
				name: mpName.trim(),
				version: mpVer,
				include: installed.filter((i) => i.enabled).map((i) => i.filename)
			});
			toast.success('Modpack generated! Download from the detail page.');
		} catch (e) {
			toast.error('Generation failed', { description: e instanceof Error ? e.message : '' });
		} finally {
			generating = false;
		}
	}

	function fmt(n: number): string {
		return n >= 1_000_000
			? `${(n / 1_000_000).toFixed(1)}M`
			: n >= 1_000
				? `${(n / 1_000).toFixed(1)}K`
				: `${n}`;
	}
	function fmtVersions(vs: string[]): string {
		return vs.slice(0, 2).join(', ') + (vs.length > 2 ? '\u2026' : '');
	}

	const typeOpts = [
		{ value: '', label: 'All types' },
		{ value: 'mod', label: 'Mod' },
		{ value: 'plugin', label: 'Plugin' },
		{ value: 'datapack', label: 'Datapack' },
		{ value: 'modpack', label: 'Modpack' },
		{ value: 'shader', label: 'Shader' }
	];
	const loaderOpts = [
		{ value: '', label: 'Any loader' },
		{ value: 'fabric', label: 'Fabric' },
		{ value: 'forge', label: 'Forge' },
		{ value: 'neoforge', label: 'NeoForge' },
		{ value: 'quilt', label: 'Quilt' },
		{ value: 'paper', label: 'Paper' },
		{ value: 'purpur', label: 'Purpur' },
		{ value: 'spigot', label: 'Spigot' },
		{ value: 'waterfall', label: 'Waterfall' },
		{ value: 'velocity', label: 'Velocity' }
	];
	const sortOpts = [
		{ value: 'relevance', label: 'Relevance' },
		{ value: 'downloads', label: 'Downloads' },
		{ value: 'follows', label: 'Follows' },
		{ value: 'newest', label: 'Newest' },
		{ value: 'updated', label: 'Updated' }
	];
	const extraVersions = $derived.by(() => {
		return ['1.21.4', '1.21.3', '1.21.1', '1.20.6', '1.20.4', '1.20.1'].filter(
			(v) => v !== mcVersion
		);
	});
	const typeLabel = $derived(typeOpts.find((o) => o.value === filterType)?.label ?? 'All types');
	const loaderLabel = $derived(
		loaderOpts.find((o) => o.value === filterLoader)?.label ?? 'Any loader'
	);
	const versionLabel = $derived(filterVersion || 'Any version');
	const sortLabel = $derived(sortOpts.find((o) => o.value === filterSort)?.label ?? 'Relevance');
</script>

<div class="mx-auto max-w-4xl px-6 py-6">
	<button
		onclick={() => goto(`/servers/${id}`)}
		class="mb-4 text-sm text-muted-foreground hover:text-foreground"
		><ArrowLeftIcon class="inline size-4" /> Back</button
	>
	<div class="mb-4 flex items-center gap-2">
		<PuzzleIcon class="size-5" />
		<h1 class="text-xl font-semibold">Mods / Plugins</h1>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<RefreshCwIcon class="size-8 animate-spin text-muted-foreground" />
		</div>
	{:else}
		<Tabs.Root bind:value={tab}>
			<Tabs.List class="mb-4">
				<Tabs.Trigger value="installed"
					><PackageIcon class="size-4" /> Installed ({installed.length})</Tabs.Trigger
				>
				<Tabs.Trigger value="browse"
					><SearchIcon class="size-4" /> Browse
					{#if installQueue.length > 0}
						<Badge class="ml-1 text-[10px] px-1 py-0">{installQueue.length}</Badge>
					{/if}
				</Tabs.Trigger>
				<Tabs.Trigger value="modpack"><BoxIcon class="size-4" /> Modpack</Tabs.Trigger>
			</Tabs.List>

			<Tabs.Content value="installed">
				{#if installed.length === 0}
					<Card.Root size="sm"
						><Card.Content class="py-8 text-center text-sm text-muted-foreground"
							><PackageIcon class="mx-auto mb-2 size-8" />
							<p>Nothing installed. Browse Modrinth to find mods/plugins.</p></Card.Content
						></Card.Root
					>
				{:else}
					<Card.Root size="sm">
						<div class="overflow-x-auto">
							<table class="w-full text-sm">
								<thead
									><tr class="border-b text-left text-xs text-muted-foreground"
										><th class="px-4 py-3 font-medium">Name</th><th class="px-4 py-3 font-medium"
											>File</th
										><th class="px-4 py-3 font-medium">Size</th><th class="px-4 py-3 font-medium"
											>Status</th
										><th class="px-4 py-3 text-right font-medium">Actions</th></tr
									></thead
								>
								<tbody>
									{#each installed as m}
										<tr class="border-b last:border-0 hover:bg-muted/30">
											<td class="px-4 py-3"
												><div class="flex items-center gap-2">
													<FileIcon class="size-4 text-muted-foreground" /><span class="font-medium"
														>{m.name}</span
													>
												</div></td
											>
											<td class="px-4 py-3 font-mono text-xs text-muted-foreground">{m.filename}</td
											>
											<td class="px-4 py-3 text-muted-foreground">{m.size_human}</td>
											<td class="px-4 py-3"
												>{#if m.enabled}<Badge variant="default" class="gap-1 text-xs"
														><CheckCircleIcon class="size-3" /> Loaded</Badge
													>{:else}<Badge variant="secondary" class="gap-1 text-xs"
														><XCircleIcon class="size-3" /> Disabled</Badge
													>{/if}</td
											>
											<td class="px-4 py-3 text-right"
												><div class="flex items-center justify-end gap-1">
													<Button
														variant="ghost"
														size="icon-xs"
														onclick={() => toggleMod(m.filename, m.enabled)}
														title={m.enabled ? 'Disable' : 'Enable'}
														>{#if m.enabled}<XCircleIcon class="size-3" />{:else}<CheckCircleIcon
																class="size-3"
															/>{/if}</Button
													>
													<Button
														variant="ghost"
														size="icon-xs"
														class="text-destructive hover:text-destructive"
														onclick={() => removeMod(m.filename)}
														title="Remove"><Trash2Icon class="size-3" /></Button
													>
												</div></td
											>
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
					<div class="relative flex-1">
						<SearchIcon
							class="absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
						/><Input
							bind:value={searchQuery}
							placeholder="Search Modrinth (or leave empty for popular)…"
							onkeydown={(e) => e.key === 'Enter' && search()}
							class="pl-10"
						/>
					</div>
					<Button onclick={() => search()} disabled={searching}
						>{#if searching}<RefreshCwIcon class="size-4 animate-spin" />{:else}<SearchIcon
								class="size-4"
							/>{/if} Search</Button
					>
				</div>
				<!-- Filters -->
				<div class="mb-4 flex flex-wrap items-center gap-2">
					<DropdownMenu.DropdownMenu>
						<DropdownMenu.Trigger>
							<Button variant="outline" size="sm" class="gap-1 text-xs"
								>{typeLabel} <ChevronDownIcon class="size-3" /></Button
							>
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
							<Button variant="outline" size="sm" class="gap-1 text-xs"
								>{loaderLabel} <ChevronDownIcon class="size-3" /></Button
							>
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
							<Button variant="outline" size="sm" class="gap-1 text-xs"
								>{versionLabel} <ChevronDownIcon class="size-3" /></Button
							>
						</DropdownMenu.Trigger>
						<DropdownMenu.Content>
							<DropdownMenu.RadioGroup bind:value={filterVersion}>
								<DropdownMenu.RadioItem value="">Any version</DropdownMenu.RadioItem>
								{#if mcVersion}
									<DropdownMenu.RadioItem value={mcVersion}
										>MC {mcVersion} (server)</DropdownMenu.RadioItem
									>
								{/if}
								{#each extraVersions as v}
									<DropdownMenu.RadioItem value={v}>{v}</DropdownMenu.RadioItem>
								{/each}
							</DropdownMenu.RadioGroup>
						</DropdownMenu.Content>
					</DropdownMenu.DropdownMenu>
					<DropdownMenu.DropdownMenu>
						<DropdownMenu.Trigger>
							<Button variant="outline" size="sm" class="gap-1 text-xs"
								>{sortLabel} <ChevronDownIcon class="size-3" /></Button
							>
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
				<div
					class="overflow-y-auto px-8 pt-8 pb-6"
					style="max-height: {installQueue.length > 0 ? '55dvh' : '65dvh'}; mask-image: linear-gradient(to bottom, transparent 0%, black 10%, black 90%, transparent 100%); -webkit-mask-image: linear-gradient(to bottom, transparent 0%, black 10%, black 90%, transparent 100%);"
					onscroll={(e) => {
						const el = e.currentTarget as HTMLElement;
						if (loadingMore || !hasMore || searching) return;
						if (el.scrollHeight - el.scrollTop - el.clientHeight < 300) {
							search(false);
						}
					}}
				>
					{#if searching && searchResults.length === 0}
						<div class="flex items-center justify-center py-16">
							<RefreshCwIcon class="size-8 animate-spin text-muted-foreground" />
						</div>
					{:else if searchResults.length > 0}
						<div class="grid grid-cols-1 gap-4 lg:grid-cols-2">
							{#each searchResults as r}
								<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
								<div
									class="group flex cursor-pointer overflow-hidden rounded-xl border bg-card shadow-xs transition-all hover:border-accent hover:shadow-md"
									onclick={() => goto(`/servers/${id}/mods/${r.slug}`)}
									onkeydown={(e) => e.key === 'Enter' && goto(`/servers/${id}/mods/${r.slug}`)}
									role="link"
									tabindex="0"
								>
									<!-- Full-height thumbnail -->
									<div
										class="relative flex w-24 shrink-0 items-center justify-center overflow-hidden bg-muted/40 sm:w-28"
									>
										{#if r.icon_url}
											<img
												src={r.icon_url}
												alt={r.title}
												class="size-16 object-contain sm:size-20"
												onerror={(e) => {
													(e.target as HTMLImageElement).style.display = 'none';
													(e.target as HTMLImageElement).nextElementSibling?.classList.remove(
														'hidden'
													);
												}}
											/>
											<PackageIcon class="hidden size-8 text-muted-foreground" />
										{:else}
											<PackageIcon class="size-8 text-muted-foreground" />
										{/if}
										<Badge variant="secondary" class="absolute bottom-1.5 left-1.5 text-[10px]"
											>{r.project_type}</Badge
										>
									</div>
									<!-- Content -->
									<div class="flex min-w-0 flex-1 flex-col justify-between p-3 sm:p-4">
										<div class="space-y-1">
											<div class="flex items-start justify-between gap-2">
												<h3 class="line-clamp-1 text-sm leading-tight font-semibold">{r.title}</h3>
											</div>
											<p class="line-clamp-2 text-xs leading-relaxed text-muted-foreground">
												{r.description}
											</p>
											<div class="flex flex-wrap items-center gap-1.5 pt-0.5">
												<Badge variant="outline" class="text-[10px]"
													>{fmt(r.downloads)} downloads</Badge
												>
												{#each r.loaders.slice(0, 3) as l}
													<Badge variant="outline" class="text-[10px]">{l}</Badge>
												{/each}
											</div>
										</div>
										<div
											class="mt-2 flex items-center justify-between gap-2"
											onclick={(e) => e.stopPropagation()}
											onkeydown={(e) => e.stopPropagation()}
											role="none"
										>
											<span class="truncate text-[10px] text-muted-foreground"
												>{fmtVersions(r.game_versions)}</span
											>
											<div class="flex shrink-0 items-center gap-1">
												<Button
													size="sm"
													class="h-7 gap-1 px-2 text-xs"
													variant={isInQueue(r.slug) ? "secondary" : "default"}
													onclick={() => toggleQueue({ slug: r.slug, title: r.title, icon_url: r.icon_url })}
												>
													{#if isInQueue(r.slug)}
														<CheckCircleIcon class="size-3" />
														Queued
													{:else}
														<PlusIcon class="size-3" />
														Queue
													{/if}
												</Button>
												<a href={r.page_url} target="_blank" rel="noopener noreferrer"
													><Button variant="ghost" size="icon-sm" class="size-7"
														><ExternalLinkIcon class="size-3.5" /></Button
													></a
												>
											</div>
										</div>
									</div>
								</div>
							{/each}
						</div>
						{#if loadingMore}
							<div class="flex items-center justify-center py-6">
								<RefreshCwIcon class="size-5 animate-spin text-muted-foreground" />
							</div>
						{:else if !hasMore}
							<p class="py-4 text-center text-xs text-muted-foreground">All results loaded</p>
						{:else}
							<div class="h-1" />
						{/if}
					{:else if searchQuery}
						<Card.Root size="sm"
							><Card.Content class="py-12 text-center text-sm text-muted-foreground"
								><SearchIcon class="mx-auto mb-2 size-8" />
								<p>No results.</p></Card.Content
							></Card.Root
						>
					{:else}
						<Card.Root size="sm"
							><Card.Content class="py-12 text-center text-sm text-muted-foreground"
								><RefreshCwIcon class="mx-auto mb-2 size-8 animate-spin text-muted-foreground" />
								<p>Loading popular mods…</p></Card.Content
							></Card.Root
						>
					{/if}
				</div>

				<!-- Floating install bar -->
				{#if installQueue.length > 0}
					<div class="sticky bottom-0 -mx-6 -mb-6 mt-2 border-t bg-background/95 backdrop-blur-sm px-6 py-3">
						<div class="flex items-center justify-between gap-3">
							<div class="flex items-center gap-2 text-sm">
								<ListIcon class="size-4 text-muted-foreground" />
								<span class="font-medium">{installQueue.length}</span>
								<span class="text-muted-foreground">{installQueue.length === 1 ? 'mod queued' : 'mods queued'}</span>
								<button onclick={clearQueue} class="text-xs text-muted-foreground hover:text-foreground underline ml-2">
									Clear
								</button>
							</div>
							<div class="flex items-center gap-2">
								{#if installErrors.length > 0}
									<div class="text-xs text-destructive">{installErrors.length} failed</div>
								{/if}
								{#if isInstallingAll}
									<div class="flex items-center gap-2 text-sm text-muted-foreground">
										<LoaderCircleIcon class="size-4 animate-spin" />
										{installProgress.current} / {installProgress.total}
									</div>
								{/if}
								<Button onclick={installAll} disabled={isInstallingAll || installQueue.length === 0} size="sm">
									{#if isInstallingAll}<RefreshCwIcon class="size-3.5 animate-spin" />{:else}<DownloadIcon class="size-3.5" />{/if}
									{isInstallingAll ? "Installing…" : `Install All (${installQueue.length})`}
								</Button>
							</div>
						</div>
					</div>
				{/if}
			</Tabs.Content>

			<Tabs.Content value="modpack">
				<div class="grid gap-4 lg:grid-cols-2">
					<div class="space-y-3">
						<Input placeholder="Modpack name" bind:value={mpName} />
						<div class="grid grid-cols-2 gap-3">
							<Input placeholder="1.0.0" bind:value={mpVer} />
							<div
								class="flex h-9 items-center rounded-md border bg-muted/30 px-2.5 text-sm text-muted-foreground"
							>
								{provider || '?'}
							</div>
						</div>
						<Button onclick={genModpack} disabled={!mpName.trim() || generating} class="w-full"
							>{#if generating}<RefreshCwIcon class="size-4 animate-spin" />{:else}<BoxIcon
									class="size-4"
								/>{/if} Generate</Button
						>
					</div>
					<Card.Root size="sm">
						<Card.Content class="py-6 text-center text-sm text-muted-foreground"
							><BoxIcon class="mx-auto mb-2 size-8" />
							<p>
								Generates a .mrpack file from installed mods/plugins. Download from the server
								detail page.
							</p></Card.Content
						>
					</Card.Root>
				</div>
			</Tabs.Content>
		</Tabs.Root>
	{/if}
</div>
