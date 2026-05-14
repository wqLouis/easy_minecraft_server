<script lang="ts">
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";
	import { goto } from "$app/navigation";
	import {
		ArrowLeftIcon,
		ServerIcon,
		RefreshCwIcon,
		DownloadIcon,
		CheckCircleIcon,
		AlertCircleIcon,
	} from "@lucide/svelte";
	import { Button } from "$lib/components/ui/button/index.js";
	import { Input } from "$lib/components/ui/input/index.js";
	import * as Card from "$lib/components/ui/card/index.js";
	import { Badge } from "$lib/components/ui/badge/index.js";
	import { isAuthenticated, getApi } from "$lib/api";



	// ── Steps ────────────────────────────────────────────

	type Step = "pick-software" | "configure" | "creating";
	let step = $state<Step>("pick-software");

	// ── Providers & versions ─────────────────────────────

	let providers = $state<{ name: string; label: string }[]>([]);
	let loadingProviders = $state(true);
	let providerError = $state("");

	let versions = $state<string[]>([]);
	let loadingVersions = $state(false);

	let selectedProvider = $state("");
	let selectedVersion = $state("");
	let versionInfo = $state<{
		name: string;
		mc_version: string;
		build: string;
		download_url: string;
		sha1: string | null;
		java_version: string | null;
	} | null>(null);
	let fetchingInfo = $state(false);

	// ── Form fields ──────────────────────────────────────

	let serverId = $state("");
	let serverName = $state("");
	let serverDir = $state("");
	let jarPath = $state("");
	let javaPath = $state("/usr/bin/java");
	let minMemory = $state("1G");
	let maxMemory = $state("4G");
	let jvmArgs = $state("-XX:+UseG1GC");

	// ── Lifecycle ────────────────────────────────────────

	onMount(() => {
		if (!isAuthenticated()) {
			toast.error("Authentication required to create servers");
			goto("/");
			return;
		}
		loadProviders();
	});

	async function loadProviders() {
		loadingProviders = true;
		providerError = "";
		try {
			const api = getApi();
			const result = await api.listProviders();
			providers = result;
		} catch (e: unknown) {
			providerError = e instanceof Error ? e.message : "Failed to load providers";
		} finally {
			loadingProviders = false;
		}
	}

	async function onProviderChange(provider: string) {
		selectedProvider = provider;
		selectedVersion = "";
		versionInfo = null;
		versions = [];
		if (!provider) return;
		loadingVersions = true;
		try {
			const api = getApi();
			versions = await api.listVersions(provider);
		} catch (e: unknown) {
			toast.error("Failed to load versions", {
				description: e instanceof Error ? e.message : "Unknown error",
			});
		} finally {
			loadingVersions = false;
		}
	}

	async function onVersionChange(version: string) {
		selectedVersion = version;
		versionInfo = null;
		if (!version || !selectedProvider) return;
		fetchingInfo = true;
		try {
			const api = getApi();
			const info = await api.versionInfo(selectedProvider, version);
			versionInfo = info;

			// Auto-fill derived fields
			const nameSlug = selectedProvider + "-" + version.replace(/\./g, "-");
			serverId = nameSlug;
			const label = providers.find(p => p.name === selectedProvider)?.label ?? selectedProvider;
			serverName = `${label} ${version}`;
			serverDir = `/srv/minecraft/${nameSlug}`;
			jarPath = info.download_url;
		} catch (e: unknown) {
			toast.error("Failed to fetch version info", {
				description: e instanceof Error ? e.message : "Unknown error",
			});
		} finally {
			fetchingInfo = false;
		}
	}

	function handleNextStep() {
		if (!selectedProvider || !selectedVersion) {
			toast.error("Please select a software and version");
			return;
		}
		step = "configure";
	}

	// ── Submit ───────────────────────────────────────────

	async function handleCreate() {
		if (!serverId.trim() || !serverName.trim()) return;
		step = "creating";
		try {
			const api = getApi();
			await api.createServer({
				id: serverId.trim(),
				name: serverName.trim(),
				jar_path: jarPath.trim() || "/srv/minecraft/server.jar",
				java_path: javaPath.trim() || "/usr/bin/java",
				min_memory: minMemory,
				max_memory: maxMemory,
				server_dir: serverDir.trim() || `/srv/minecraft/${serverId.trim()}`,
				jvm_args: jvmArgs.trim() ? jvmArgs.split(/\s+/).filter(Boolean) : undefined,
			});
			toast.success(`Server "${serverName}" created`);
			goto("/");
		} catch (e: unknown) {
			toast.error("Failed to create server", {
				description: e instanceof Error ? e.message : "Unknown error",
			});
			step = "configure";
		}
	}
</script>

<div class="mx-auto max-w-2xl px-6 py-6">
	<!-- Back link -->
	<button onclick={() => goto("/")} class="mb-4 flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground">
		<ArrowLeftIcon class="size-4" /> Back to Servers
	</button>

	<div class="mb-6">
		<h1 class="text-xl font-semibold">New Server</h1>
		<p class="text-sm text-muted-foreground">Choose server software and configure your instance.</p>
	</div>

	<!-- Step indicator -->
	<div class="mb-6 flex items-center gap-2 text-sm">
		<Badge variant={step === "pick-software" ? "default" : "secondary"}>1. Pick Software</Badge>
		<span class="text-muted-foreground">→</span>
		<Badge variant={step === "configure" ? "default" : "secondary"}>2. Configure</Badge>
		<span class="text-muted-foreground">→</span>
		<Badge variant={step === "creating" ? "default" : "secondary"}>3. Create</Badge>
	</div>

	<!-- STEP 1: Pick Software -->
	{#if step === "pick-software"}
		<Card.Root size="sm">
			<Card.Header>
				<Card.Title>Server Software</Card.Title>
				<Card.Description>Select a provider and Minecraft version.</Card.Description>
			</Card.Header>
			<Card.Content class="grid gap-4">
				{#if loadingProviders}
					<div class="flex items-center justify-center py-8"><RefreshCwIcon class="size-6 animate-spin text-muted-foreground" /></div>
				{:else if providerError}
					<div class="flex flex-col items-center gap-3 py-4 text-center">
						<AlertCircleIcon class="size-8 text-destructive" />
						<p class="text-sm text-destructive">{providerError}</p>
						<Button variant="outline" size="sm" onclick={loadProviders}>Retry</Button>
					</div>
				{:else}
					<!-- Provider -->
					<div class="grid gap-2">
						<label for="provider" class="text-sm font-medium">Provider</label>
						<select
							id="provider"
							value={selectedProvider}
							onchange={(e) => onProviderChange((e.target as HTMLSelectElement).value)}
							class="dark:bg-input/30 border-input focus-visible:border-ring focus-visible:ring-ring/50 h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm shadow-xs focus-visible:ring-3 outline-none"
						>
							<option value="">— Select —</option>
							{#each providers as p}
								<option value={p.name}>{p.label}</option>
							{/each}
						</select>
					</div>

					<!-- Version -->
					{#if selectedProvider}
						<div class="grid gap-2">
							<label for="version" class="text-sm font-medium">Minecraft Version</label>
							{#if loadingVersions}
								<div class="flex items-center gap-2 text-sm text-muted-foreground">
									<RefreshCwIcon class="size-4 animate-spin" /> Loading versions…
								</div>
							{:else}
								<select
									id="version"
									value={selectedVersion}
									onchange={(e) => onVersionChange((e.target as HTMLSelectElement).value)}
									class="dark:bg-input/30 border-input focus-visible:border-ring focus-visible:ring-ring/50 h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm shadow-xs focus-visible:ring-3 outline-none"
								>
									<option value="">— Select —</option>
									{#each versions as v}
										<option value={v}>{v}</option>
									{/each}
								</select>
							{/if}
						</div>
					{/if}

					<!-- Version info -->
					{#if fetchingInfo}
						<div class="flex items-center gap-2 text-sm text-muted-foreground">
							<RefreshCwIcon class="size-4 animate-spin" /> Fetching download info…
						</div>
					{/if}

					{#if versionInfo}
						<div class="rounded-md border bg-muted/30 p-3 text-sm">
							<div class="flex items-center gap-2 text-green-600 dark:text-green-400">
								<CheckCircleIcon class="size-4" />
								<span class="font-medium">{versionInfo.name}</span>
							</div>
							<div class="mt-2 grid grid-cols-2 gap-1 text-xs text-muted-foreground">
								<span>Build: {versionInfo.build}</span>
								<span>Java: {versionInfo.java_version ?? "auto"}</span>
							</div>
						</div>
					{/if}
				{/if}
			</Card.Content>
			<Card.Footer>
				<Button onclick={handleNextStep} disabled={!selectedProvider || !selectedVersion} class="w-full">
					Next — Configure
				</Button>
			</Card.Footer>
		</Card.Root>

	<!-- STEP 2: Configure -->
	{:else if step === "configure"}
		<Card.Root size="sm">
			<Card.Header>
				<Card.Title>Instance Configuration</Card.Title>
				<Card.Description>
					{providers.find(p => p.name === selectedProvider)?.label ?? selectedProvider} {selectedVersion}
					— adjust the settings below.
				</Card.Description>
			</Card.Header>
			<Card.Content class="grid gap-4">
				<div class="grid grid-cols-2 gap-4">
					<div class="grid gap-2">
						<label for="server-id" class="text-sm font-medium">Server ID</label>
						<Input id="server-id" placeholder="my-server" bind:value={serverId} />
					</div>
					<div class="grid gap-2">
						<label for="server-name" class="text-sm font-medium">Server Name</label>
						<Input id="server-name" placeholder="My Server" bind:value={serverName} />
					</div>
				</div>
				<div class="grid gap-2">
					<label for="server-dir" class="text-sm font-medium">Server Directory</label>
					<Input id="server-dir" placeholder="/srv/minecraft/my-server" bind:value={serverDir} />
				</div>
				<div class="grid gap-2">
					<label for="jar-path" class="text-sm font-medium">JAR Path / Download URL</label>
					<Input id="jar-path" placeholder="/srv/minecraft/server.jar" bind:value={jarPath} />
				</div>
				<div class="grid gap-2">
					<label for="java-path" class="text-sm font-medium">Java Binary</label>
					<Input id="java-path" placeholder="/usr/bin/java" bind:value={javaPath} />
				</div>
				<div class="grid grid-cols-2 gap-4">
					<div class="grid gap-2">
						<label for="min-memory" class="text-sm font-medium">Min Memory</label>
						<Input id="min-memory" placeholder="1G" bind:value={minMemory} />
					</div>
					<div class="grid gap-2">
						<label for="max-memory" class="text-sm font-medium">Max Memory</label>
						<Input id="max-memory" placeholder="4G" bind:value={maxMemory} />
					</div>
				</div>
				<div class="grid gap-2">
					<label for="jvm-args" class="text-sm font-medium">JVM Arguments (space-separated)</label>
					<Input id="jvm-args" placeholder="-XX:+UseG1GC" bind:value={jvmArgs} />
				</div>
			</Card.Content>
			<Card.Footer class="flex justify-between">
				<Button variant="outline" onclick={() => step = "pick-software"}>
					← Back
				</Button>
				<Button onclick={handleCreate} disabled={!serverId.trim() || !serverName.trim()}>
					<ServerIcon class="size-4" /> Create Server
				</Button>
			</Card.Footer>
		</Card.Root>

	<!-- STEP 3: Creating -->
	{:else if step === "creating"}
		<div class="flex flex-col items-center justify-center gap-4 py-20 text-center">
			<RefreshCwIcon class="size-10 animate-spin text-muted-foreground" />
			<div>
				<h2 class="text-lg font-medium">Creating Server…</h2>
				<p class="mt-1 text-sm text-muted-foreground">{serverName}</p>
			</div>
		</div>
	{/if}
</div>
