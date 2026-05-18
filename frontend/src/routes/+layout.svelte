<script lang="ts">
  import "./layout.css";
  import favicon from "$lib/assets/favicon.svg";
  import { Toaster } from "$lib/components/ui/sonner/index.js";
  import { ModeWatcher } from "mode-watcher";
  import Sidebar from "$lib/components/sidebar.svelte";
  import LogViewer from "$lib/components/log-viewer.svelte";
  import { logViewerState } from "$lib/stores";

  let { children } = $props();
  // Keep local state that syncs with the shared store for two-way binding
  let lv = $state($logViewerState);
  $effect(() => { lv = $logViewerState; });
  $effect(() => { logViewerState.set(lv); });
</script>

<svelte:head><title>EazyMC Manager</title><link rel="icon" href={favicon} /></svelte:head>
<ModeWatcher />

<div class="flex min-h-dvh">
  <Sidebar />
  <main class="flex-1 overflow-auto">{@render children()}</main>
</div>

<Toaster />
<LogViewer bind:open={lv.open} serverName={lv.serverName} serverId={lv.serverId} />
