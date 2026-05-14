<script lang="ts">
	import './layout.css';
	import favicon from '$lib/assets/favicon.svg';
	import { Toaster } from '$lib/components/ui/sonner/index.js';
	import { ModeWatcher } from 'mode-watcher';
	import Sidebar from '$lib/components/sidebar.svelte';
	import CreateServerDialog from '$lib/components/create-server-dialog.svelte';
	import LogViewer from '$lib/components/log-viewer.svelte';
	import { showCreateDialog, logViewerState } from '$lib/stores';
	import { isAuthenticated, getApi } from '$lib/api';
	import { toast } from 'svelte-sonner';
	import { onMount } from 'svelte';
	import type { MinecraftServer, CreateServerRequest } from '$lib/types';

	let { children } = $props();

	// --- Global server operations (shared across routes) ---

	let servers = $state<MinecraftServer[]>([]);
	let createDialogOpen = $state(false);
	let logViewer = $state({ open: false, serverId: '', serverName: '' });
	let logs = $state<string[]>([]);
	let logsLoading = $state(false);

	// Sync with stores
	showCreateDialog.subscribe((val) => (createDialogOpen = val));
	logViewerState.subscribe((val) => (logViewer = val));

	function setServers(s: MinecraftServer[]) {
		servers = s;
	}

	async function handleCreateServer(data: CreateServerRequest) {
		try {
			const api = getApi();
			const server = await api.createServer(data);
			servers = [...servers, server];
			createDialogOpen = false;
			showCreateDialog.set(false);
			toast.success('Server created', {
				description: `"${server.name}" is being set up.`,
			});
		} catch (e: unknown) {
			toast.error('Failed to create server', {
				description: e instanceof Error ? e.message : 'Unknown error',
			});
		}
	}

	async function loadLogs(id: string) {
		logsLoading = true;
		try {
			const api = getApi();
			logs = await api.getServerLogs(id);
		} catch (e: unknown) {
			toast.error('Failed to load logs', {
				description: e instanceof Error ? e.message : 'Unknown error',
			});
		} finally {
			logsLoading = false;
		}
	}

	function handleLogViewerOpen(id: string, name: string) {
		logViewer = { open: true, serverId: id, serverName: name };
		logViewerState.set(logViewer);
		logs = [];
		loadLogs(id);
	}

	function handleLogRefresh() {
		if (logViewer.serverId) loadLogs(logViewer.serverId);
	}
</script>

<svelte:head><title>EazyMC Manager</title><link rel="icon" href={favicon} /></svelte:head>

<ModeWatcher />

<div class="flex min-h-dvh">
	<Sidebar />
	<main class="flex-1 overflow-auto">
		{@render children()}
	</main>
</div>

<Toaster />

<!-- Global Create Server Dialog -->
<CreateServerDialog
	bind:open={createDialogOpen}
	onSubmit={handleCreateServer}
/>

<!-- Global Log Viewer -->
<LogViewer
	bind:open={logViewer.open}
	serverName={logViewer.serverName}
	{logs}
	loading={logsLoading}
	onRefresh={handleLogRefresh}
/>
