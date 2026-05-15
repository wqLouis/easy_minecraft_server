import { writable, derived } from "svelte/store";
import { getActiveEndpoint, isConfigured, isAuthenticated, getEndpoints } from "$lib/api";

export const logViewerState = writable<{ open: boolean; serverId: string; serverName: string }>({
  open: false, serverId: "", serverName: "",
});

export const _connectionVersion = writable(0);
export function refreshConnectionState() { _connectionVersion.update((n) => n + 1) }

export const endpoints = derived(_connectionVersion, () => getEndpoints());
export const activeEndpoint = derived(_connectionVersion, () => getActiveEndpoint());
export const configured = derived(_connectionVersion, () => isConfigured());
export const authenticated = derived(_connectionVersion, () => isAuthenticated());
