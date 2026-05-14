<script lang="ts">
	import { page } from "$app/stores";
	import { ServerIcon, HomeIcon, SettingsIcon, UsersIcon, PlusIcon, LogOutIcon, CableIcon, KeyIcon, MoonIcon, SunIcon, GlobeIcon } from "@lucide/svelte";
	import { Button } from "$lib/components/ui/button/index.js";
	import { Badge } from "$lib/components/ui/badge/index.js";
	import { goto } from "$app/navigation";
import { activeEndpoint, configured, authenticated, refreshConnectionState } from "$lib/stores";
	import { clearAuth, resetApi, getActiveEndpointId } from "$lib/api";
	import { mode, toggleMode } from "mode-watcher";

	let sidebarOpen = $state(true);

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
	class="bg-sidebar text-sidebar-foreground flex h-dvh flex-col border-r transition-all duration-200"
	class:w-56={sidebarOpen}
	class:w-14={!sidebarOpen}
>
	<!-- Logo / Header -->
	<div class="flex h-14 items-center gap-2 border-b px-3">
		<button onclick={() => (sidebarOpen = !sidebarOpen)} class="shrink-0 rounded-md p-1 hover:bg-sidebar-accent">
			<ServerIcon class="size-5" />
		</button>
		{#if sidebarOpen}
			<span class="text-sm font-semibold whitespace-nowrap">EazyMC Manager</span>
		{/if}
	</div>

	<!-- Navigation -->
	<nav class="flex-1 space-y-1 px-2 py-3">
		{#each navItems as item}
			<a
				href={item.href}
				data-slot="sidebar-nav-link"
				class="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground aria-[current=page]:bg-sidebar-accent aria-[current=page]:text-sidebar-accent-foreground"
				aria-current={$page.url.pathname === item.href ? "page" : undefined}
			>
				<item.icon class="size-4 shrink-0" />
				{#if sidebarOpen}
					<span>{item.label}</span>
				{/if}
			</a>
		{/each}

		<!-- New Server button -->
		<button
			onclick={handleNewServer}
			class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground disabled:opacity-40"
			disabled={!$authenticated}
		>
			<PlusIcon class="size-4 shrink-0" />
			{#if sidebarOpen}
				<span>New Server</span>
			{/if}
		</button>
	</nav>

	<!-- Footer: connection status + theme toggle + logout -->
	<div class="border-t p-2">
		{#if sidebarOpen}
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
				{:else}
					<div class="flex items-center gap-2 text-xs text-sidebar-foreground/60">
						<CableIcon class="size-3" />
						<span>No endpoint selected</span>
					</div>
				{/if}
			</div>
		{/if}
		<!-- Theme toggle -->
		<button
			onclick={toggleMode}
			class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-sidebar-foreground/70 transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
			title={mode.current === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
		>
			{#if mode.current === 'dark'}
				<SunIcon class="size-4 shrink-0" />
			{:else}
				<MoonIcon class="size-4 shrink-0" />
			{/if}
			{#if sidebarOpen}
				<span>{mode.current === 'dark' ? 'Light Mode' : 'Dark Mode'}</span>
			{/if}
		</button>

		{#if $authenticated}
			<button
				onclick={handleLogout}
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-sidebar-foreground/70 transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
			>
				<LogOutIcon class="size-4 shrink-0" />
				{#if sidebarOpen}
					<span>Logout</span>
				{/if}
			</button>
		{/if}
	</div>
</aside>
