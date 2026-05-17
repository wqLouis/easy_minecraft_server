// ── Endpoint management (localStorage) ─────────────────────────

export interface Endpoint { id: string; name: string; url: string }

const E = "mc_endpoints", A = "mc_active_endpoint";
const _keys: Record<string, string> = {};

function ls(): Storage | null {
  return typeof localStorage !== "undefined" ? localStorage : null;
}
function load<T>(k: string, fallback: T): T {
  try { const r = ls()?.getItem(k); return r ? JSON.parse(r) : fallback } catch { return fallback }
}
function save(k: string, v: unknown) { try { ls()?.setItem(k, JSON.stringify(v)) } catch {} }

export function getEndpoints(): Endpoint[] { return load(E, []) }
export function addEndpoint(name: string, url: string): Endpoint {
  const ep: Endpoint = { id: crypto.randomUUID(), name, url: url.replace(/\/+$/, "") };
  const list = [...getEndpoints(), ep];
  save(E, list);
  return ep;
}
export function removeEndpoint(id: string) {
  save(E, getEndpoints().filter((e) => e.id !== id));
  delete _keys[id];
  if (getActiveEndpointId() === id) clearActiveEndpoint();
}
export function updateEndpoint(id: string, u: Partial<Pick<Endpoint, "name" | "url">>): Endpoint | null {
  const list = getEndpoints();
  const idx = list.findIndex((e) => e.id === id);
  if (idx === -1) return null;
  list[idx] = { ...list[idx], ...u, url: u.url ? u.url.replace(/\/+$/, "") : list[idx].url };
  save(E, list);
  return list[idx];
}

export function setActiveEndpoint(id: string) { ls()?.setItem(A, id) }
export function getActiveEndpointId(): string | null { return ls()?.getItem(A) ?? null }
export function clearActiveEndpoint() { ls()?.removeItem(A) }
export function getActiveEndpoint(): Endpoint | null {
  const id = getActiveEndpointId();
  return id ? getEndpoints().find((e) => e.id === id) ?? null : null;
}
export function isConfigured(): boolean { return getActiveEndpoint() !== null }

// ── Auth (in-memory only) ─────────────────────────────────────

export function getApiKey(id?: string): string | null { return _keys[id ?? getActiveEndpointId() ?? ""] ?? null }
export function setApiKey(key: string, id?: string) { const eid = id ?? getActiveEndpointId(); if (eid) _keys[eid] = key }
export function clearAuth(id?: string) {
  id ? delete _keys[id] : Object.keys(_keys).forEach((k) => delete _keys[k]);
}
export function isAuthenticated(id?: string): boolean { return getApiKey(id) !== null }

// ── API client ────────────────────────────────────────────────

export class ApiRequestError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.name = "ApiRequestError";
    this.status = status;
  }
}

export class ApiClient {
  constructor(
    private baseUrl: string,
    private endpointId: string,
  ) {
    this.baseUrl = baseUrl.replace(/\/+$/, "");
  }

  async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    const headers: Record<string, string> = { "Content-Type": "application/json" };
    const key = _keys[this.endpointId];
    if (key) headers["Authorization"] = `Bearer ${key}`;
    // Replay defense: attach timestamp + unique nonce to every request
    headers["X-Timestamp"] = Math.floor(Date.now() / 1000).toString();
    headers["X-Nonce"] = crypto.randomUUID();
    const res = await fetch(`${this.baseUrl}${path}`, {
      method,
      headers,
      body: body ? JSON.stringify(body) : undefined,
    });
    if (!res.ok) {
      let msg = `HTTP ${res.status}`;
      try { const e = await res.json(); msg = e.message ?? e.error ?? msg } catch {}
      throw new ApiRequestError(res.status, msg);
    }
    return res.status === 204 ? (undefined as T) : res.json();
  }

  get = <T>(p: string) => this.request<T>("GET", p);
  post = <T>(p: string, b?: unknown) => this.request<T>("POST", p, b);
  put = <T>(p: string, b?: unknown) => this.request<T>("PUT", p, b);
  del = <T>(p: string) => this.request<T>("DELETE", p);
}

let _client: ApiClient | null = null;
let _clientEid: string | null = null;

export function getApi(): ApiClient {
  const ep = getActiveEndpoint();
  if (!ep) throw new Error("No active endpoint configured.");
  if (!_client || _clientEid !== ep.id) {
    _client = new ApiClient(ep.url, ep.id);
    _clientEid = ep.id;
  }
  return _client;
}

export function resetApi() { _client = null; _clientEid = null }
