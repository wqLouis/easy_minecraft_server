import type { MinecraftServer, CreateServerRequest, UserProfile, ServerLogEntry, JsonSchema } from "$lib/types";

// ─── Types ────────────────────────────────────────────────────────

export interface Endpoint {
	id: string;
	name: string;
	url: string;
}

// ─── Storage keys ─────────────────────────────────────────────────

const STORAGE_ENDPOINTS = "mc_endpoints";
const STORAGE_ACTIVE = "mc_active_endpoint";

// ─── In-memory API keys (per endpoint id, never in localStorage) ──

const _apiKeys: Record<string, string> = {};

// ─── Endpoint CRUD (localStorage) ─────────────────────────────────

export function getEndpoints(): Endpoint[] {
	if (typeof localStorage === "undefined") return [];
	try {
		const raw = localStorage.getItem(STORAGE_ENDPOINTS);
		return raw ? (JSON.parse(raw) as Endpoint[]) : [];
	} catch {
		return [];
	}
}

function saveEndpoints(list: Endpoint[]): void {
	localStorage.setItem(STORAGE_ENDPOINTS, JSON.stringify(list));
}

export function addEndpoint(name: string, url: string): Endpoint {
	const list = getEndpoints();
	const endpoint: Endpoint = {
		id: crypto.randomUUID?.() ?? Math.random().toString(36).slice(2),
		name,
		url: url.replace(/\/+$/, ""),
	};
	list.push(endpoint);
	saveEndpoints(list);
	return endpoint;
}

export function removeEndpoint(id: string): void {
	const list = getEndpoints().filter((e) => e.id !== id);
	saveEndpoints(list);
	// Clear in-memory key for this endpoint
	delete _apiKeys[id];
	// If the active endpoint was removed, clear it
	if (getActiveEndpointId() === id) {
		clearActiveEndpoint();
	}
}

export function updateEndpoint(id: string, updates: Partial<Pick<Endpoint, "name" | "url">>): Endpoint | null {
	const list = getEndpoints();
	const idx = list.findIndex((e) => e.id === id);
	if (idx === -1) return null;
	list[idx] = { ...list[idx], ...updates, url: updates.url ? updates.url.replace(/\/+$/, "") : list[idx].url };
	saveEndpoints(list);
	return list[idx];
}

// ─── Active endpoint ──────────────────────────────────────────────

export function setActiveEndpoint(id: string): void {
	localStorage.setItem(STORAGE_ACTIVE, id);
}

export function getActiveEndpointId(): string | null {
	if (typeof localStorage === "undefined") return null;
	return localStorage.getItem(STORAGE_ACTIVE);
}

export function clearActiveEndpoint(): void {
	localStorage.removeItem(STORAGE_ACTIVE);
}

export function getActiveEndpoint(): Endpoint | null {
	const id = getActiveEndpointId();
	if (!id) return null;
	return getEndpoints().find((e) => e.id === id) ?? null;
}

export function isConfigured(): boolean {
	return getActiveEndpoint() !== null;
}

// ─── Auth (in-memory only) ────────────────────────────────────────

export function getApiKey(endpointId?: string): string | null {
	const id = endpointId ?? getActiveEndpointId();
	return id ? (_apiKeys[id] ?? null) : null;
}

export function setApiKey(key: string, endpointId?: string): void {
	const id = endpointId ?? getActiveEndpointId();
	if (id) _apiKeys[id] = key;
}

export function clearAuth(endpointId?: string): void {
	if (endpointId) {
		delete _apiKeys[endpointId];
	} else {
		// Clear all keys
		for (const k of Object.keys(_apiKeys)) delete _apiKeys[k];
	}
}

export function isAuthenticated(endpointId?: string): boolean {
	return getApiKey(endpointId) !== null;
}

// ─── API client ───────────────────────────────────────────────────

class ApiClient {
	private baseUrl: string;
	private endpointId: string;

	constructor(baseUrl: string, endpointId: string) {
		this.baseUrl = baseUrl.replace(/\/+$/, "");
		this.endpointId = endpointId;
	}

	private async request<T>(
		method: string,
		path: string,
		body?: unknown,
		skipAuth = false,
	): Promise<T> {
		const url = `${this.baseUrl}${path}`;
		const headers: Record<string, string> = {
			"Content-Type": "application/json",
		};

		if (!skipAuth) {
			const key = _apiKeys[this.endpointId];
			if (key) {
				headers["Authorization"] = `Bearer ${key}`;
			}
		}

		const res = await fetch(url, {
			method,
			headers,
			body: body ? JSON.stringify(body) : undefined,
		});

		if (!res.ok) {
			let message = `HTTP ${res.status}`;
			try {
				const err = await res.json();
				message = err.message ?? err.error ?? message;
			} catch {
				// ignore
			}
			throw new ApiRequestError(res.status, message);
		}

		if (res.status === 204) {
			return undefined as T;
		}

		return res.json();
	}

	async getMe(): Promise<{ user: UserProfile }> {
		return this.request("GET", "/api/auth/me");
	}

	async listServers(): Promise<MinecraftServer[]> {
		return this.request("GET", "/api/instances");
	}

	async getServer(id: string): Promise<MinecraftServer> {
		return this.request("GET", `/api/instances/${id}`);
	}

	async createServer(data: CreateServerRequest): Promise<MinecraftServer> {
		return this.request("POST", "/api/instances", data);
	}

	async deleteServer(id: string): Promise<void> {
		return this.request("DELETE", `/api/instances/${id}`);
	}

	async startServer(id: string): Promise<void> {
		return this.request("POST", `/api/instances/${id}/start`);
	}

	async stopServer(id: string): Promise<void> {
		return this.request("POST", `/api/instances/${id}/stop`);
	}

	async getServerLogs(id: string, tail = 100): Promise<string[]> {
		const res = await this.request<{ id: string; logs: string[]; count: number }>("GET", `/api/instances/${id}/logs?tail=${tail}`);
		return res.logs;
	}

	// --- Providers / Versions ---

	async listProviders(): Promise<{ name: string; label: string }[]> {
		return this.request("GET", "/api/providers");
	}

	async listVersions(provider: string): Promise<string[]> {
		const res = await this.request<{ provider: string; versions: string[] }>("GET", `/api/providers/${provider}/versions`);
		return res.versions;
	}

	async versionInfo(provider: string, version: string): Promise<{
		name: string;
		mc_version: string;
		build: string;
		download_url: string;
		sha1: string | null;
		java_version: string | null;
	}> {
		return this.request("GET", `/api/providers/${provider}/versions/${version}`);
	}

	async getSettings(): Promise<Record<string, unknown>> {
		return this.request("GET", "/api/settings");
	}

	async getSettingsSchema(): Promise<JsonSchema> {
		return this.request("GET", "/api/settings/schema");
	}

	async updateSettings(data: Record<string, unknown>): Promise<Record<string, unknown>> {
		return this.request("PUT", "/api/settings", data);
	}

	// --- User Management (sudo) ---

	async listUsers(): Promise<UserProfile[]> {
		return this.request("GET", "/api/users");
	}

	async registerUser(email: string): Promise<{ api_key: string; user: UserProfile }> {
		return this.request("POST", "/api/auth/register", { email });
	}

	async updateUser(id: string, data: { email: string }): Promise<UserProfile> {
		return this.request("PUT", `/api/users/${id}`, data);
	}

	async deleteUser(id: string): Promise<void> {
		return this.request("DELETE", `/api/users/${id}`);
	}
}

export class ApiRequestError extends Error {
	status: number;
	constructor(status: number, message: string) {
		super(message);
		this.name = "ApiRequestError";
		this.status = status;
	}
}

let clientInstance: ApiClient | null = null;
let clientEndpointId: string | null = null;

export function getApi(): ApiClient {
	const ep = getActiveEndpoint();
	if (!ep) {
		throw new Error("No active endpoint configured. Go to Connection settings.");
	}
	if (!clientInstance || clientEndpointId !== ep.id) {
		clientInstance = new ApiClient(ep.url, ep.id);
		clientEndpointId = ep.id;
	}
	return clientInstance;
}

export function resetApi(): void {
	clientInstance = null;
	clientEndpointId = null;
}
