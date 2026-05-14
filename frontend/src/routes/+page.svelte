<script lang="ts">
	import { onMount } from "svelte";
	import { goto } from "$app/navigation";
import { toast } from "svelte-sonner";
	import {
		ServerIcon,
		PlusIcon,
		RefreshCwIcon,
		AlertCircleIcon,
		GlobeIcon,
		KeyIcon,
		LogOutIcon,
		WifiIcon,
	} from "@lucide/svelte";
	import { Button } from "$lib/components/ui/button/index.js";
	import { Input } from "$lib/components/ui/input/index.js";
	import * as Card from "$lib/components/ui/card/index.js";
	import ServerCard from "$lib/components/server-card.svelte";
	import { showCreateDialog, logViewerState, refreshConnectionState } from "$lib/stores";
	import type { MinecraftServer } from "$lib/types";
	import {
		getApi,
		getActiveEndpoint,
		getActiveEndpointId,
		setActiveEndpoint,
		isConfigured,
		isAuthenticated,
		setApiKey,
		clearAuth,
		resetApi,
		getEndpoints,
		type Endpoint,
	} from "$lib/api";

	// --- Page phase ---
	type Phase = "no-endpoint" | "auth-endpoint" | "dashboard";
	let phase = $state<Phase>("no-endpoint");

	let endpoints = $state<Endpoint[]>([]);
	let authKey = $state("");
	let authLoading = $state(false);
	let authError = $state("");

	let servers = $state<MinecraftServer[]>([]);
	let dashboardLoading = $state(true);
	let dashboardError = $state("");

	onMount(() => {
		if (!isConfigured()) {
			const all = getEndpoints();
			if (all.length > 0) endpoints = all;
			phase = "no-endpoint";
		} else if (!isAuthenticated()) {
			phase = "auth-endpoint";
		} else {
			phase = "dashboard";
			loadServers();
		}
	});

	// ── Endpoint selection ───────────────────────────────

	function handleSelectEndpoint(id: string) {
		setActiveEndpoint(id);
		resetApi();
		authKey = "";
		authError = "";
		if (isAuthenticated()) {
			phase = "dashboard";
			loadServers();
		} else {
			phase = "auth-endpoint";
		}
	}

	// ── Auth ─────────────────────────────────────────────

	async function handleAuth() {
		const key = authKey.trim();
		if (!key) return;
		const epId = getActiveEndpointId();
		if (!epId) return;
		authLoading = true;
		authError = "";
		setApiKey(key, epId);
		resetApi();
		try {
			const api = getApi();
			await api.getMe();
			phase = "dashboard";
			refreshConnectionState();
			loadServers();
		} catch (e: unknown) {
			clearAuth(epId);
			authError = e instanceof Error ? e.message : "Authentication failed";
		} finally {
			authLoading = false;
		}
	}

	function handleSkipAuth() {
		phase = "dashboard";
		loadServers();
	}

	// ── Dashboard ────────────────────────────────────────

	async function loadServers() {
		if (!isConfigured()) { dashboardLoading = false; return; }
		dashboardLoading = true;
		dashboardError = "";
		try {
			const api = getApi();
			servers = await api.listServers();
		} catch (e: unknown) {
			dashboardError = e instanceof Error ? e.message : "Failed to load servers";
			toast.error("Failed to load servers", { description: dashboardError });
		} finally {
			dashboardLoading = false;
		}
	}

	async function handleStartServer(id: string) {
		try {
			const api = getApi();
			await api.startServer(id);
			servers = servers.map((s) => (s.id === id ? { ...s, running: true } : s));
			toast.success("Server started");
		} catch (e: unknown) {
			toast.error("Failed to start server", {
				description: e instanceof Error ? e.message : "Unknown error",
			});
		}
	}

	async function handleStopServer(id: string) {
		try {
			const api = getApi();
			await api.stopServer(id);
			servers = servers.map((s) => (s.id === id ? { ...s, running: false } : s));
			toast.success("Server stopped");
		} catch (e: unknown) {
			toast.error("Failed to stop server", {
				description: e instanceof Error ? e.message : "Unknown error",
			});
		}
	}

	async function handleDeleteServer(id: string) {
		if (!confirm("Delete this server? This cannot be undone.")) return;
		try {
			const api = getApi();
			await api.deleteServer(id);
			servers = servers.filter((s) => s.id !== id);
			toast.success("Server deleted");
		} catch (e: unknown) {
			toast.error("Failed to delete server", {
				description: e instanceof Error ? e.message : "Unknown error",
			});
		}
	}

	function handleViewLogs(id: string) {
		const server = servers.find((s) => s.id === id);
		if (!server) return;
		logViewerState.set({ open: true, serverId: id, serverName: server.name });
	}

	function handleLogout() {
		const epId = getActiveEndpointId();
		if (epId) clearAuth(epId);
		resetApi();
		refreshConnectionState();
		phase = "auth-endpoint";
		servers = [];
	}

	const runningCount = $derived(servers.filter((s) => s.running).length);
	const activeEp = $derived(getActiveEndpoint());
</script>

<div class="mx-auto flex flex-col px-6 py-6">
	<!-- NO ENDPOINT -->
	{#if phase === "no-endpoint"}
		<div class="flex flex-1 items-center justify-center" style="min-height: calc(100dvh - 3rem)">
			<Card.Root size="sm" class="w-full max-w-md">
				<Card.Header>
					<div class="mb-2 flex items-center gap-2"><ServerIcon class="size-5" /><Card.Title>Welcome</Card.Title></div>
					<Card.Description>
						{#if endpoints.length > 0}Select an endpoint to connect to.{:else}No endpoints configured — go to the Connection tab.{/if}
					</Card.Description>
				</Card.Header>
				{#if endpoints.length > 0}
					<Card.Content class="grid gap-2">
						{#each endpoints as ep (ep.id)}
							<button onclick={() => handleSelectEndpoint(ep.id)}
								class="flex items-center gap-3 rounded-md border p-3 text-left text-sm transition-colors hover:bg-accent/50">
								<GlobeIcon class="size-4 shrink-0 text-muted-foreground" />
								<div class="min-w-0 flex-1">
									<span class="block truncate font-medium">{ep.name}</span>
									<span class="block truncate text-xs text-muted-foreground">{ep.url}</span>
								</div>
							</button>
						{/each}
					</Card.Content>
				{/if}
				<Card.Footer>
					<Button variant="outline" class="w-full" onclick={() => goto("/connection")}>
						<GlobeIcon class="size-4" /> Manage Endpoints
					</Button>
				</Card.Footer>
			</Card.Root>
		</div>

	<!-- AUTH -->
	{:else if phase === "auth-endpoint"}
		<div class="flex flex-1 items-center justify-center" style="min-height: calc(100dvh - 3rem)">
			<Card.Root size="sm" class="w-full max-w-md">
				<Card.Header>
					<div class="mb-2 flex items-center gap-2"><ServerIcon class="size-5" /><Card.Title>Authentication</Card.Title></div>
					<Card.Description>Enter API key for <span class="font-medium">{activeEp?.name ?? "endpoint"}</span></Card.Description>
				</Card.Header>
				<Card.Content class="grid gap-4">
					<Input id="home-auth-key" type="password" placeholder="Paste your API key" bind:value={authKey} disabled={authLoading} />
					{#if authError}
						<div class="flex items-center gap-2 rounded-md border border-red-300 bg-red-50 p-3 text-sm text-red-700 dark:border-red-800 dark:bg-red-950 dark:text-red-400">
							<AlertCircleIcon class="size-4 shrink-0" />{authError}
						</div>
					{/if}
					<button class="text-xs text-muted-foreground underline underline-offset-2 hover:text-foreground"
						onclick={() => { clearAuth(getActiveEndpointId() ?? undefined); resetApi(); phase = "no-endpoint"; }}>
						Switch endpoint
					</button>
				</Card.Content>
				<Card.Footer class="flex-col gap-2">
					<Button class="w-full" onclick={handleAuth} disabled={!authKey.trim() || authLoading}>
						{authLoading ? "Authenticating…" : "Authenticate"}
					</Button>
					<Button variant="ghost" class="w-full" onclick={handleSkipAuth}>Skip — read-only mode</Button>
				</Card.Footer>
			</Card.Root>
		</div>

	<!-- DASHBOARD -->
	{:else if phase === "dashboard"}
		<div class="mb-6 flex items-center justify-between">
			<div>
				<h1 class="text-xl font-semibold">Minecraft Servers</h1>
				<p class="text-sm text-muted-foreground">{activeEp?.name ?? "Servers"}{isAuthenticated() ? "" : " — read-only"}</p>
			</div>
			<div class="flex items-center gap-2">
				<Button variant="outline" size="sm" onclick={loadServers} disabled={dashboardLoading}>
					<RefreshCwIcon class={['size-4', dashboardLoading ? 'animate-spin' : ''].join(' ')} /> Refresh
				</Button>
				<Button size="sm" onclick={() => showCreateDialog.set(true)} disabled={!isAuthenticated()}>
					<PlusIcon class="size-4" /> New Server
				</Button>
				{#if isAuthenticated()}
					<Button variant="ghost" size="sm" onclick={handleLogout}><LogOutIcon class="size-4" /> Logout</Button>
				{/if}
			</div>
		</div>

		<div class="mb-6 grid grid-cols-2 gap-3 sm:grid-cols-2">
			<Card.Root size="sm" class="py-3"><Card.Content class="text-center"><p class="text-2xl font-bold">{servers.length}</p><p class="text-xs text-muted-foreground">Total</p></Card.Content></Card.Root>
			<Card.Root size="sm" class="border-green-200 py-3 dark:border-green-900"><Card.Content class="text-center"><p class="text-2xl font-bold text-green-600 dark:text-green-400">{runningCount}</p><p class="text-xs text-muted-foreground">Running</p></Card.Content></Card.Root>
		</div>

		{#if dashboardLoading}
			<div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
		{:else if servers.length === 0}
			<div class="flex flex-col items-center justify-center gap-4 py-20 text-center">
				<ServerIcon class="size-12 text-muted-foreground" />
				<div><h2 class="text-lg font-medium">No Servers Yet</h2><p class="mt-1 text-sm text-muted-foreground">Create your first server.</p></div>
				<Button onclick={() => goto("/servers/new")}><PlusIcon class="size-4" /> Create Server</Button>
			</div>
		{:else}
			<div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
				{#each servers as server (server.id)}
					<ServerCard {server} onStart={handleStartServer} onStop={handleStopServer} onDelete={handleDeleteServer} onViewLogs={handleViewLogs} />
				{/each}
			</div>
		{/if}
	{/if}
</div>
