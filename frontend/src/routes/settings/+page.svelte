<script lang="ts">
	import { onMount } from "svelte";
	import { goto } from "$app/navigation";
import { toast } from "svelte-sonner";
	import { SaveIcon, RefreshCwIcon, AlertCircleIcon, GlobeIcon, KeyIcon } from "@lucide/svelte";
	import { Button } from "$lib/components/ui/button/index.js";
	import { Input } from "$lib/components/ui/input/index.js";
	import { Badge } from "$lib/components/ui/badge/index.js";
	import * as Card from "$lib/components/ui/card/index.js";
	import { isAuthenticated, isConfigured, getApi, getActiveEndpoint, getActiveEndpointId, setApiKey, clearAuth, resetApi } from "$lib/api";
import { refreshConnectionState } from "$lib/stores";
	import type { JsonSchema, JsonSchemaProperty } from "$lib/types";

	// --- State ---

	type PagePhase = "no-endpoint" | "no-auth" | "loading" | "ready" | "error";
	let phase = $state<PagePhase>("loading");

	let schema: JsonSchema | null = $state(null);
	let settings: Record<string, unknown> = $state({});
	let saving = $state(false);
	let errorMsg = $state("");

	// Auth form
	let authKey = $state("");
	let authLoading = $state(false);

	onMount(() => {
		if (!isConfigured()) {
			phase = "no-endpoint";
		} else if (!isAuthenticated()) {
			phase = "no-auth";
		} else {
			loadSettings();
		}
	});

	async function loadSettings() {
		phase = "loading";
		errorMsg = "";
		try {
			const api = getApi();
			const [schemaData, settingsData] = await Promise.all([
				api.getSettingsSchema(),
				api.getSettings(),
			]);
			schema = schemaData;
			settings = settingsData;
			phase = "ready";
		} catch (e: unknown) {
			errorMsg = e instanceof Error ? e.message : "Failed to load settings";
			phase = "error";
			toast.error("Failed to load settings", { description: errorMsg });
		}
	}

	async function handleSave() {
		saving = true;
		try {
			const api = getApi();
			const updated = await api.updateSettings(settings);
			settings = updated;
			toast.success("Settings saved");
		} catch (e: unknown) {
			toast.error("Failed to save settings", {
				description: e instanceof Error ? e.message : "Unknown error",
			});
		} finally {
			saving = false;
		}
	}

	function updateField(key: string, value: unknown) {
		settings = { ...settings, [key]: value };
	}

	// --- Auth ---

	async function handleAuth() {
		const key = authKey.trim();
		if (!key) return;
		const epId = getActiveEndpointId();
		if (!epId) return;
		authLoading = true;
		setApiKey(key, epId);
		resetApi();
		try {
			const api = getApi();
			await api.getMe();
			refreshConnectionState();
			loadSettings();
		} catch (e: unknown) {
			clearAuth(epId);
			toast.error("Authentication failed", {
				description: e instanceof Error ? e.message : "Unknown error",
			});
		} finally {
			authLoading = false;
		}
	}

	const activeEp = $derived(getActiveEndpoint());
</script>

<div class="mx-auto max-w-2xl px-6 py-6">
	<div class="mb-6">
		<h1 class="text-xl font-semibold">Settings</h1>
		<p class="text-sm text-muted-foreground">
			Backend configuration for
			<span class="font-medium">{activeEp?.name ?? "selected endpoint"}</span>
		</p>
	</div>

	<!-- No endpoint selected -->
	{#if phase === "no-endpoint"}
		<Card.Root size="sm" class="mx-auto max-w-md">
			<Card.Header>
				<Card.Title>No Endpoint Selected</Card.Title>
				<Card.Description>
					Go to the Connection tab to select an active endpoint first.
				</Card.Description>
			</Card.Header>
			<Card.Footer>
				<Button variant="outline" class="w-full" onclick={() => goto("/connection")}>
					<GlobeIcon class="size-4" />
					Open Connection
				</Button>
			</Card.Footer>
		</Card.Root>

	<!-- Not authenticated -->
	{:else if phase === "no-auth"}
		<Card.Root size="sm" class="mx-auto max-w-md">
			<Card.Header>
				<Card.Title>Authentication Required</Card.Title>
				<Card.Description>
					Settings for <span class="font-medium">{activeEp?.name}</span> require sudo privileges.
				</Card.Description>
			</Card.Header>
			<Card.Content class="grid gap-4">
				<div class="grid gap-2">
					<label for="settings-auth-key" class="text-sm font-medium">API Key</label>
					<Input id="settings-auth-key" type="password" placeholder="Paste your API key" bind:value={authKey} disabled={authLoading} />
				</div>
			</Card.Content>
			<Card.Footer>
				<Button class="w-full" onclick={handleAuth} disabled={!authKey.trim() || authLoading}>
					{authLoading ? "Authenticating…" : "Authenticate"}
				</Button>
			</Card.Footer>
		</Card.Root>

	<!-- Loading / Error / Ready -->
	{:else if phase === "loading"}
		<div class="flex items-center justify-center py-20">
			<RefreshCwIcon class="size-8 animate-spin text-muted-foreground" />
		</div>

	{:else if phase === "error"}
		<div class="flex flex-col items-center justify-center gap-4 py-20 text-center">
			<AlertCircleIcon class="size-12 text-muted-foreground" />
			<div>
				<h2 class="text-lg font-medium">Failed to Load</h2>
				<p class="mt-1 text-sm text-muted-foreground">{errorMsg}</p>
			</div>
			<Button onclick={loadSettings}>Retry</Button>
		</div>

	{:else if phase === "ready" && schema}
		<form onsubmit={(e) => { e.preventDefault(); handleSave(); }} class="grid gap-6">
			{#each Object.entries(schema.properties ?? {}) as [key, prop] (key)}
				{@const typedProp = prop as JsonSchemaProperty}
				<div class="grid gap-2">
					<label for={"setting-" + key} class="flex items-center gap-1 text-sm font-medium">
						{prop.title ?? humanizeKey(key)}
						{#if schema.required?.includes(key)}
							<span class="text-destructive">*</span>
						{/if}
					</label>
					{#if typedProp.description}
						<p class="text-xs text-muted-foreground">{typedProp.description}</p>
					{/if}

					{#if typedProp.type === "boolean"}
						<div class="flex items-center gap-2">
							<input
								id={"setting-" + key}
								type="checkbox"
								checked={settings[key] as boolean ?? false}
								onchange={(e) => updateField(key, (e.target as HTMLInputElement).checked)}
								class="border-input h-4 w-4 rounded border accent-primary"
							/>
							<span class="text-sm">Enabled</span>
						</div>
					{:else if typedProp.type === "integer" || typedProp.type === "number"}
						<Input
							id={"setting-" + key}
							type="number"
							min={typedProp.minimum}
							max={typedProp.maximum}
							step={typedProp.type === "integer" ? 1 : 0.01}
							value={settings[key] as number ?? ""}
							oninput={(e) => updateField(key, typedProp.type === "integer" ? parseInt((e.target as HTMLInputElement).value, 10) : parseFloat((e.target as HTMLInputElement).value))}
						/>
					{:else if typedProp.type === "string" && typedProp.enum}
						<select
							id={"setting-" + key}
							value={settings[key] as string ?? ""}
							onchange={(e) => updateField(key, (e.target as HTMLSelectElement).value)}
							class="dark:bg-input/30 border-input focus-visible:border-ring focus-visible:ring-ring/50 h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm shadow-xs focus-visible:ring-3 outline-none"
						>
							{#each typedProp.enum as opt}
								<option value={opt}>{opt}</option>
							{/each}
						</select>
					{:else}
						<Input
							id={"setting-" + key}
							type="text"
							value={settings[key] as string ?? ""}
							oninput={(e) => updateField(key, (e.target as HTMLInputElement).value)}
						/>
					{/if}
				</div>
			{/each}

			<div class="flex items-center gap-2 pt-2">
				<Button type="submit" disabled={saving}>
					{#if saving}
						<RefreshCwIcon class="size-4 animate-spin" />
					{:else}
						<SaveIcon class="size-4" />
					{/if}
					Save Settings
				</Button>
				<Button variant="outline" type="button" onclick={loadSettings} disabled={saving}>
					<RefreshCwIcon class="size-4" />
					Reset
				</Button>
			</div>
		</form>
	{/if}
</div>

<script lang="ts" module>
	function humanizeKey(key: string): string {
		return key
			.replace(/_/g, " ")
			.replace(/\b\w/g, (c) => c.toUpperCase());
	}
</script>
