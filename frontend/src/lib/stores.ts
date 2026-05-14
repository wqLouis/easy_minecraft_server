import { writable, derived } from "svelte/store";
import {
	getActiveEndpoint,
	isConfigured,
	isAuthenticated,
	getEndpoints,
} from "$lib/api";
import type { Endpoint } from "$lib/api";

/** Global state for opening the Create Server dialog from anywhere (e.g. sidebar) */
export const showCreateDialog = writable(false);

/** Global state for the Log Viewer */
export const logViewerState = writable<{
	open: boolean;
	serverId: string;
	serverName: string;
}>({ open: false, serverId: "", serverName: "" });

// ── Reactive connection state (syncs with api.ts module) ──────────

/** Counter bumped whenever endpoints or active endpoint changes */
export const _connectionVersion = writable(0);

/** Bump to signal all connection-state consumers to refresh */
export function refreshConnectionState(): void {
	_connectionVersion.update((n) => n + 1);
}

/** Reactive: list of all endpoints */
export const endpoints = derived(_connectionVersion, () => getEndpoints());

/** Reactive: the active endpoint */
export const activeEndpoint = derived(_connectionVersion, () => getActiveEndpoint());

/** Reactive: whether an active endpoint is configured */
export const configured = derived(_connectionVersion, () => isConfigured());

/** Reactive: whether the active endpoint has an API key in memory */
export const authenticated = derived(_connectionVersion, () => isAuthenticated());
