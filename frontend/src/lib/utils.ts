import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

// ── Reusable type utilities (used by shadcn-svelte components) ──

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithoutChild<T> = T extends { child?: any } ? Omit<T, "child"> : T;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithoutChildren<T> = T extends { children?: any } ? Omit<T, "children"> : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & { ref?: U | null };

// ── Shared formatting helpers ──────────────────────────────────

/** Format byte count into human-readable string (e.g. "1.5 MB") */
export function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}

/** Format large numbers compactly (e.g. 1500 -> "1.5K") */
export function fmtCompact(n: number): string {
	return n >= 1_000_000
		? `${(n / 1_000_000).toFixed(1)}M`
		: n >= 1_000
			? `${(n / 1_000).toFixed(1)}K`
			: `${n}`;
}

/** Format version list for display (e.g. ["1.20", "1.21"] -> "1.20, 1.21…") */
export function fmtVersions(vs: string[], max = 2): string {
	return vs.slice(0, max).join(", ") + (vs.length > max ? "…" : "");
}

/** Convert snake_case key to human-readable label (e.g. "max_players" -> "Max Players") */
export function humanize(key: string): string {
	return key.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}

/** Parse a string as a number (integer or float) for form inputs, returns the raw string if invalid */
export function parseNum(raw: string, type: string): number | string {
	if (raw === "") return "";
	const n = type === "integer" ? parseInt(raw, 10) : parseFloat(raw);
	return isNaN(n) ? raw : n;
}
