import type { Snippet } from "svelte";

export interface MinecraftServer {
  id: string;
  name: string;
  running: boolean;
  provider?: string;
  version?: string;
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
  provider: string;
  version: string;
  jar_path: string;
  java_path: string;
  min_memory: string;
  max_memory: string;
  server_dir: string;
  jvm_args?: string[];
}

export interface UserProfile {
  id: string;
  username: string;
  is_sudoer: boolean;
  created_at: string;
}

export interface JsonSchemaProperty {
  type?: string;
  format?: string;
  title?: string;
  description?: string;
  default?: unknown;
  minimum?: number;
  maximum?: number;
  enum?: string[];
}

export interface JsonSchema {
  title?: string;
  description?: string;
  type?: string;
  properties?: Record<string, JsonSchemaProperty>;
  required?: string[];
}
