<script lang="ts">
	import { page } from "$app/stores";
	import { HomeIcon, SettingsIcon, UsersIcon, PlusIcon, LogOutIcon, CableIcon, MoonIcon, SunIcon, GlobeIcon, PanelLeftCloseIcon, PanelLeftIcon, TriangleAlertIcon } from "@lucide/svelte";
	import { goto } from "$app/navigation";
	import { activeEndpoint, configured, authenticated, refreshConnectionState } from "$lib/stores";
	import { clearAuth, resetApi, getActiveEndpointId } from "$lib/api";
	import { mode, toggleMode } from "mode-watcher";

	// ─── Persist collapse state in localStorage ───────────────────────

	const STORAGE_KEY = "mc_sidebar_collapsed";

	function loadCollapsed(): boolean {
		if (typeof localStorage === "undefined") return false;
		try {
			return localStorage.getItem(STORAGE_KEY) === "true";
		} catch {
			return false;
		}
	}

	function saveCollapsed(val: boolean): void {
		try {
			if (val) localStorage.setItem(STORAGE_KEY, "true");
			else localStorage.removeItem(STORAGE_KEY);
		} catch { /* ignore */ }
	}

	let sidebarCollapsed = $state(loadCollapsed());

	$effect(() => {
		saveCollapsed(sidebarCollapsed);
	});

	const navItems = [
		{ href: "/", label: "Home", icon: HomeIcon },
		{ href: "/connection", label: "Connection", icon: GlobeIcon },
		{ href: "/users", label: "Users", icon: UsersIcon },
		{ href: "/settings", label: "Settings", icon: SettingsIcon },
	];

	function handleNewServer() {
		goto("/servers/new");
	}

	function handleLogout() {
		const epId = getActiveEndpointId();
		if (epId) clearAuth(epId);
		resetApi();
		refreshConnectionState();
	}
</script>

<aside
	class="bg-sidebar text-sidebar-foreground group relative flex h-dvh flex-col border-r transition-all duration-200 ease-in-out"
	class:w-60={!sidebarCollapsed}
	class:w-16={sidebarCollapsed}
>
	<!-- Logo / Header -->
	<div class="flex h-14 items-center gap-3 border-b px-3">
		<button
			onclick={() => (sidebarCollapsed = !sidebarCollapsed)}
			class="shrink-0 rounded-md p-1.5 hover:bg-sidebar-accent transition-colors"
			title={sidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar'}
		>
			{#if sidebarCollapsed}
				<PanelLeftIcon class="size-5" />
			{:else}
				<PanelLeftCloseIcon class="size-5" />
			{/if}
		</button>
		{#if !sidebarCollapsed}
			<span class="truncate text-sm font-semibold">EazyMC Manager</span>
		{/if}
	</div>

	<!-- Navigation -->
	<nav class="flex-1 space-y-0.5 overflow-y-auto px-2 py-3">
		{#each navItems as item}
			<a
				href={item.href}
				data-slot="sidebar-nav-link"
				class="group/link relative flex items-center gap-3 rounded-md px-2 py-2 text-sm transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground aria-[current=page]:bg-sidebar-accent aria-[current=page]:text-sidebar-accent-foreground {sidebarCollapsed ? 'justify-center' : ''}"
				aria-current={$page.url.pathname === item.href ? "page" : undefined}
				title={sidebarCollapsed ? item.label : undefined}
			>
				<item.icon class="size-5 shrink-0" />
				{#if !sidebarCollapsed}
					<span class="truncate">{item.label}</span>
				{/if}
			</a>
		{/each}

		<!-- Separator -->
		<hr class="my-2 border-sidebar-border/50" />

		<!-- New Server button -->
		<button
			onclick={handleNewServer}
			class="group/link relative flex w-full items-center gap-3 rounded-md px-2 py-2 text-sm transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground disabled:opacity-40 {sidebarCollapsed ? 'justify-center' : ''}"
			disabled={!$authenticated}
			title={sidebarCollapsed ? 'New Server' : undefined}
		>
			<PlusIcon class="size-5 shrink-0" />
			{#if !sidebarCollapsed}
				<span class="truncate">New Server</span>
			{/if}
		</button>
	</nav>

	<!-- Footer: connection status + theme toggle + logout -->
	<div class="border-t border-sidebar-border/50 p-2">
		{#if !sidebarCollapsed}
			<div class="mb-2 space-y-1 px-2">
				{#if $configured}
					<div class="flex items-center gap-2 text-xs text-sidebar-foreground/60">
						{#if $authenticated}
							<CableIcon class="size-3 shrink-0 text-green-500" />
						{:else}
							<CableIcon class="size-3 shrink-0 text-yellow-500" />
						{/if}
						<div class="min-w-0 truncate">
							<span class="block truncate font-medium text-sidebar-foreground/80">{$activeEndpoint?.name ?? "Unknown"}</span>
							<span class="block truncate">{$authenticated ? "Authenticated" : "Not authenticated"}</span>
						</div>
					</div>
					{#if $activeEndpoint?.url?.startsWith('http://')}
						<div class="flex items-start gap-1.5 rounded-md border border-amber-500/30 bg-amber-500/10 p-2 text-xs text-amber-600 dark:text-amber-400">
							<TriangleAlertIcon class="mt-0.5 size-3 shrink-0" />
							<span>Connection is not HTTPS. Enable IP whitelist for security.</span>
						</div>
					{/if}
				{:else}
					<div class="flex items-center gap-2 text-xs text-sidebar-foreground/60">
						<CableIcon class="size-3" />
						<span>No endpoint selected</span>
					</div>
				{/if}
			</div>
		{/if}

		<div class="space-y-0.5">
			<!-- Theme toggle -->
			<button
				onclick={toggleMode}
				class="flex w-full items-center gap-3 rounded-md px-2 py-2 text-sm text-sidebar-foreground/70 transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground {sidebarCollapsed ? 'justify-center' : ''}"
				title={mode.current === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
			>
				{#if mode.current === 'dark'}
					<SunIcon class="size-5 shrink-0" />
				{:else}
					<MoonIcon class="size-5 shrink-0" />
				{/if}
				{#if !sidebarCollapsed}
					<span class="truncate">{mode.current === 'dark' ? 'Light Mode' : 'Dark Mode'}</span>
				{/if}
			</button>

			{#if $authenticated}
				<button
					onclick={handleLogout}
					class="flex w-full items-center gap-3 rounded-md px-2 py-2 text-sm text-sidebar-foreground/70 transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground {sidebarCollapsed ? 'justify-center' : ''}"
					title="Logout"
				>
					<LogOutIcon class="size-5 shrink-0" />
					{#if !sidebarCollapsed}
						<span class="truncate">Logout</span>
					{/if}
				</button>
			{/if}
		</div>
	</div>
</aside>
