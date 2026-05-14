// --- Minecraft Server Types (matches backend InstanceSummary / InstanceConfig) ---

export interface MinecraftServer {
	id: string;
	name: string;
	running: boolean;
	// Full detail (from get_instance or after create)
	jar_path?: string;
	java_path?: string;
	min_memory?: string;
	max_memory?: string;
	server_dir?: string;
	jvm_args?: string[];
}

export interface CreateServerRequest {
	id: string;
	name: string;
	jar_path: string;
	java_path: string;
	min_memory: string;
	max_memory: string;
	server_dir: string;
	jvm_args?: string[];
}

export type ServerStatus = "running" | "stopped";

export interface ServerLogEntry {
	timestamp?: string;
	level?: string;
	message: string;
}

// --- Auth Types ---

export interface UserProfile {
	id: string;
	email: string;
	is_sudoer: boolean;
	created_at: string;
}

export interface ApiError {
	status: number;
	message: string;
}

// --- JSON Schema (draft 2020-12) subset for dynamic settings form ---

export interface JsonSchemaProperty {
	type?: string;
	format?: string;
	description?: string;
	default?: unknown;
	minimum?: number;
	maximum?: number;
	enum?: string[];
	[key: string]: unknown;
}

export interface JsonSchema {
	$schema?: string;
	title?: string;
	description?: string;
	type?: string;
	properties?: Record<string, JsonSchemaProperty>;
	required?: string[];
	[key: string]: unknown;
}
