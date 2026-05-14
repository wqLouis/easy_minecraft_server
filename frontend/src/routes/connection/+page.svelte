<script lang="ts">
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";
	import {
		GlobeIcon,
		PlusIcon,
		RefreshCwIcon,
		CheckCircleIcon,
		AlertCircleIcon,
		KeyIcon,
		LogOutIcon,
		Trash2Icon,
		CheckIcon,
		PencilIcon,
		XIcon,
		ServerIcon,
	} from "@lucide/svelte";
	import { Button } from "$lib/components/ui/button/index.js";
	import { Input } from "$lib/components/ui/input/index.js";
	import * as Card from "$lib/components/ui/card/index.js";
	import { Badge } from "$lib/components/ui/badge/index.js";
	import { refreshConnectionState } from "$lib/stores";
import {
	type Endpoint,
		getEndpoints,
		addEndpoint,
		removeEndpoint,
		updateEndpoint,
		setActiveEndpoint,
		getActiveEndpointId,
		getActiveEndpoint,
		getApiKey,
		setApiKey,
		clearAuth,
		isAuthenticated,
		isConfigured,
		getApi,
		resetApi,
	} from "$lib/api";

	let endpoints = $state<Endpoint[]>([]);
	let activeId = $state<string | null>(null);

	// Add form
	let newName = $state("");
	let newUrl = $state("");

	// Selected endpoint for auth
	let selectedId = $state<string | null>(null);
	let apiKeyInput = $state("");
	let authTesting = $state(false);
	let authResult: { ok: boolean; message: string } | null = $state(null);

	// Editing
	let editingId = $state<string | null>(null);
	let editName = $state("");
	let editUrl = $state("");

	onMount(() => {
		loadEndpoints();
	});

	function loadEndpoints() {
		endpoints = getEndpoints();
		activeId = getActiveEndpointId();
		// Select active endpoint by default
		if (activeId && !selectedId) {
			selectedId = activeId;
		} else if (endpoints.length > 0 && !selectedId) {
			selectedId = endpoints[0].id;
		}
	}

	const selectedEndpoint = $derived(endpoints.find((e) => e.id === selectedId) ?? null);

	// ── Add ──────────────────────────────────────────────

	function handleAdd() {
		const name = newName.trim();
		const url = newUrl.trim();
		if (!name || !url) return;
		const ep = addEndpoint(name, url);
		endpoints = getEndpoints();
		selectedId = ep.id;
		newName = "";
		newUrl = "";
		refreshConnectionState();
		toast.success(`Endpoint "${name}" added`);
	}

	// ── Remove ───────────────────────────────────────────

	function handleRemove(id: string) {
		const ep = endpoints.find((e) => e.id === id);
		if (!ep) return;
		if (!confirm(`Remove endpoint "${ep.name}"?`)) return;
		removeEndpoint(id);
		endpoints = getEndpoints();
		activeId = getActiveEndpointId();
		if (selectedId === id) selectedId = endpoints[0]?.id ?? null;
		refreshConnectionState();
		toast.success(`Endpoint "${ep.name}" removed`);
	}

	// ── Select active ────────────────────────────────────

	function handleSetActive(id: string) {
		setActiveEndpoint(id);
		activeId = id;
		resetApi();
		refreshConnectionState();
		toast.success("Active endpoint switched");
	}

	// ── Edit ─────────────────────────────────────────────

	function startEdit(ep: Endpoint) {
		editingId = ep.id;
		editName = ep.name;
		editUrl = ep.url;
	}

	function cancelEdit() {
		editingId = null;
	}

	function saveEdit(id: string) {
		const name = editName.trim();
		const url = editUrl.trim();
		if (!name || !url) return;
		updateEndpoint(id, { name, url });
		endpoints = getEndpoints();
		editingId = null;
		resetApi();
		refreshConnectionState();
		toast.success("Endpoint updated");
	}

	// ── Auth ─────────────────────────────────────────────

	async function handleAuthenticate() {
		const ep = selectedEndpoint;
		if (!ep || !apiKeyInput.trim()) return;
		authTesting = true;
		authResult = null;
		// Temporarily set key to test
		setApiKey(apiKeyInput.trim(), ep.id);
		resetApi();
		try {
			const api = getApi();
			const me = await api.getMe();
			authResult = {
				ok: true,
				message: `Authenticated as ${me.user.email}${me.user.is_sudoer ? " (sudoer)" : ""}`,
			};
			refreshConnectionState();
			toast.success("Authenticated");
		} catch (e: unknown) {
			clearAuth(ep.id);
			authResult = {
				ok: false,
				message: e instanceof Error ? e.message : "Authentication failed",
			};
		} finally {
			authTesting = false;
		}
	}

	function handleLogout() {
		const ep = selectedEndpoint;
		if (!ep) return;
		clearAuth(ep.id);
		resetApi();
		authResult = null;
		refreshConnectionState();
		toast.success("Logged out");
	}

	function selectEndpoint(id: string) {
		selectedId = id;
		authResult = null;
		apiKeyInput = "";
	}
</script>

<div class="mx-auto max-w-3xl px-6 py-6">
	<div class="mb-6">
		<h1 class="text-xl font-semibold">Connection</h1>
		<p class="text-sm text-muted-foreground">
			Manage multiple backend endpoints. Switch between them and authenticate per endpoint.
			API keys are kept in memory only and are lost on page reload.
		</p>
	</div>

	<div class="grid gap-6 lg:grid-cols-5">
		<!-- ── Endpoint list (left) ── -->
		<div class="space-y-3 lg:col-span-2">
			<Card.Root size="sm">
				<Card.Header>
					<Card.Title class="flex items-center gap-2">
						<ServerIcon class="size-4" />
						Endpoints
					</Card.Title>
				</Card.Header>
				<Card.Content class="grid gap-2">
					{#if endpoints.length === 0}
						<p class="py-4 text-center text-sm text-muted-foreground">
							No endpoints yet. Add one below.
						</p>
					{:else}
						{#each endpoints as ep (ep.id)}
							<div
								class={[
									"group flex cursor-pointer items-center gap-2 rounded-md border px-3 py-2 text-sm transition-colors",
									selectedId === ep.id ? "border-primary bg-accent/30" : "hover:bg-accent/20",
								].join(" ")}
								onclick={() => selectEndpoint(ep.id)}
								role="button"
								tabindex="0"
								onkeydown={(e) => e.key === "Enter" && selectEndpoint(ep.id)}
							>
								<!-- Active indicator -->
								<div class="shrink-0">
									{#if activeId === ep.id}
										<CheckCircleIcon class="size-4 text-green-500" />
									{:else}
										<div class="size-4 rounded-full border-2 border-muted-foreground/30"></div>
									{/if}
								</div>
								<!-- Name + URL -->
								<div class="min-w-0 flex-1">
									<div class="flex items-center gap-1.5">
										<span class="truncate font-medium">{ep.name}</span>
										{#if activeId === ep.id}
											<Badge variant="secondary" class="shrink-0 text-[10px]">active</Badge>
										{/if}
									</div>
									<p class="truncate text-xs text-muted-foreground">{ep.url}</p>
								</div>
								<!-- Auth indicator -->
								<div class="shrink-0" title={isAuthenticated(ep.id) ? "Authenticated" : "Not authenticated"}>
									<div class={["size-2 rounded-full", isAuthenticated(ep.id) ? "bg-green-500" : "bg-gray-300"].join(" ")}></div>
								</div>
							</div>
						{/each}
					{/if}
				</Card.Content>
			</Card.Root>

			<!-- Add endpoint form -->
			<Card.Root size="sm">
				<Card.Header>
					<Card.Title class="flex items-center gap-2">
						<PlusIcon class="size-4" />
						Add Endpoint
					</Card.Title>
				</Card.Header>
				<Card.Content class="grid gap-3">
					<Input placeholder="Name (e.g. Home Server)" bind:value={newName} />
					<Input type="url" placeholder="http://192.168.1.100:3000" bind:value={newUrl} />
				</Card.Content>
				<Card.Footer>
					<Button onclick={handleAdd} disabled={!newName.trim() || !newUrl.trim()} class="w-full">
						<PlusIcon class="size-4" />
						Add
					</Button>
				</Card.Footer>
			</Card.Root>
		</div>

		<!-- ── Selected endpoint details (right) ── -->
		<div class="space-y-4 lg:col-span-3">
			{#if selectedEndpoint}
				<!-- Edit / Actions bar -->
				<Card.Root size="sm">
					<Card.Content class="flex flex-wrap items-center justify-between gap-2">
						<div class="flex items-center gap-2">
							<GlobeIcon class="size-4 text-muted-foreground" />
							<span class="text-sm font-medium">{selectedEndpoint.name}</span>
							<span class="text-xs text-muted-foreground">{selectedEndpoint.url}</span>
						</div>
						<div class="flex items-center gap-1">
							<Button
								variant="outline"
								size="xs"
								onclick={() => handleSetActive(selectedEndpoint.id)}
								disabled={activeId === selectedEndpoint.id}
							>
								<CheckIcon class="size-3" />
								Set Active
							</Button>
							<Button variant="ghost" size="icon-xs" onclick={() => startEdit(selectedEndpoint)}>
								<PencilIcon class="size-3" />
							</Button>
							<Button
								variant="ghost"
								size="icon-xs"
								class="text-destructive hover:text-destructive"
								onclick={() => handleRemove(selectedEndpoint.id)}
							>
								<Trash2Icon class="size-3" />
							</Button>
						</div>
					</Card.Content>
				</Card.Root>

				<!-- Edit form -->
				{#if editingId === selectedEndpoint.id}
					<Card.Root size="sm">
						<Card.Header>
							<Card.Title>Edit Endpoint</Card.Title>
						</Card.Header>
						<Card.Content class="grid gap-3">
							<Input bind:value={editName} placeholder="Name" />
							<Input type="url" bind:value={editUrl} placeholder="URL" />
						</Card.Content>
						<Card.Footer class="flex gap-2">
							<Button variant="outline" onclick={cancelEdit}><XIcon class="size-4" /> Cancel</Button>
							<Button onclick={() => saveEdit(selectedEndpoint.id)} disabled={!editName.trim() || !editUrl.trim()}>
								<CheckIcon class="size-4" /> Save
							</Button>
						</Card.Footer>
					</Card.Root>
				{/if}

				<!-- Authentication -->
				<Card.Root size="sm">
					<Card.Header>
						<Card.Title class="flex items-center gap-2">
							<KeyIcon class="size-4" />
							Authentication
						</Card.Title>
						<Card.Description>
							API key is stored in memory only for this endpoint.
							<span class="font-medium text-amber-600 dark:text-amber-400">
								It will be lost on page reload.
							</span>
						</Card.Description>
					</Card.Header>
					<Card.Content class="grid gap-3">
						{#if isAuthenticated(selectedEndpoint.id)}
							<div class="flex items-center gap-2 rounded-md border border-green-300 bg-green-50 p-3 text-sm text-green-700 dark:border-green-800 dark:bg-green-950 dark:text-green-400">
								<CheckCircleIcon class="size-4 shrink-0" />
								Authenticated
							</div>
						{/if}

						{#if !isAuthenticated(selectedEndpoint.id)}
							<Input
								type="password"
								placeholder="Paste your API key"
								bind:value={apiKeyInput}
								disabled={authTesting}
							/>
						{/if}

						{#if authResult}
							<div
								class="flex items-center gap-2 rounded-md border p-3 text-sm"
								class:border-green-300={authResult.ok}
								class:bg-green-50={authResult.ok}
								class:text-green-700={authResult.ok}
								class:dark:border-green-800={authResult.ok}
								class:dark:bg-green-950={authResult.ok}
								class:dark:text-green-400={authResult.ok}
								class:border-red-300={!authResult.ok}
								class:bg-red-50={!authResult.ok}
								class:text-red-700={!authResult.ok}
								class:dark:border-red-800={!authResult.ok}
								class:dark:bg-red-950={!authResult.ok}
								class:dark:text-red-400={!authResult.ok}
							>
								{#if authResult.ok}
									<CheckCircleIcon class="size-4 shrink-0" />
								{:else}
									<AlertCircleIcon class="size-4 shrink-0" />
								{/if}
								{authResult.message}
							</div>
						{/if}
					</Card.Content>
					<Card.Footer class="flex gap-2">
						{#if isAuthenticated(selectedEndpoint.id)}
							<Button variant="outline" onclick={handleLogout}>
								<LogOutIcon class="size-4" />
								Logout
							</Button>
						{:else}
							<Button onclick={handleAuthenticate} disabled={!apiKeyInput.trim() || authTesting}>
								{#if authTesting}
									<RefreshCwIcon class="size-4 animate-spin" />
								{:else}
									<KeyIcon class="size-4" />
								{/if}
								Authenticate
							</Button>
						{/if}
					</Card.Footer>
				</Card.Root>
			{:else}
				<div class="flex items-center justify-center py-20 text-center">
					<div>
						<GlobeIcon class="mx-auto mb-2 size-10 text-muted-foreground" />
						<p class="text-sm text-muted-foreground">Select an endpoint from the list to configure it.</p>
					</div>
				</div>
			{/if}
		</div>
	</div>
</div>
