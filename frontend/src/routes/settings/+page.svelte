<script lang="ts">
  import { onMount } from "svelte";
  import { toast } from "svelte-sonner";
  import { SaveIcon, RefreshCwIcon, AlertCircleIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import { Separator } from "$lib/components/ui/separator/index.js";
  import { isAuthenticated, isConfigured, getApi } from "$lib/api";
import { humanize, parseNum } from "$lib/utils";
  import type { JsonSchema, JsonSchemaProperty } from "$lib/types";

  let schema: JsonSchema | null = $state(null);
  let settings: Record<string, unknown> = $state({});
  let loading = $state(true), saving = $state(false), error = $state("");

  onMount(() => {
    if (!isConfigured() || !isAuthenticated()) return;
    loadSettings();
  });

  async function loadSettings() {
    loading = true; error = "";
    try {
      const api = getApi();
      const [s, v] = await Promise.all([
        api.get<JsonSchema>("/api/settings/schema"),
        api.get<Record<string, unknown>>("/api/settings"),
      ]);
      schema = s; settings = v;
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : "Failed";
      toast.error("Failed to load settings", { description: error });
    } finally { loading = false }
  }

  async function handleSave() {
    saving = true;
    try { settings = await getApi().put<Record<string, unknown>>("/api/settings", settings); toast.success("Saved") }
    catch (e: unknown) { toast.error("Failed", { description: e instanceof Error ? e.message : "" }) }
    finally { saving = false }
  }

  function updateField(key: string, value: unknown) { settings = { ...settings, [key]: value } }
</script>

<div class="mx-auto max-w-2xl px-6 py-6">
  <div class="mb-6">
    <h1 class="text-xl font-semibold">Settings</h1>
    <p class="text-sm text-muted-foreground">Backend configuration.</p>
  </div>

  {#if loading}
    <div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
  {:else if error}
    <div class="flex flex-col items-center justify-center gap-4 py-20 text-center">
      <AlertCircleIcon class="size-12 text-muted-foreground" />
      <p class="text-sm text-muted-foreground">{error}</p>
      <Button onclick={loadSettings}>Retry</Button>
    </div>
  {:else if schema}
    <form onsubmit={(e) => { e.preventDefault(); handleSave() }} class="grid gap-6">
      <div class="space-y-5">
        {#each Object.entries(schema.properties ?? {}) as [key, prop], i (key)}
          {@const p = prop as JsonSchemaProperty}
          {@const label = p.title ?? humanize(key)}
          <div class="grid gap-2">
            <div class="flex items-center justify-between">
              <label for={"s-" + key} class="flex items-center gap-1.5 text-sm font-medium">
                {label}{#if schema.required?.includes(key)}<span class="text-destructive" title="Required">*</span>{/if}
              </label>
              {#if p.type === "integer" || p.type === "number"}
                <Badge variant="outline" class="text-[10px]">{p.type === "integer" ? "Integer" : "Number"}</Badge>
              {/if}
            </div>
            {#if p.description}<p class="text-xs text-muted-foreground">{p.description}</p>{/if}

            {#if p.type === "boolean"}
              <div class="flex items-center gap-3">
                <button type="button" role="switch" aria-label={label} aria-checked={!!settings[key]} onclick={() => updateField(key, !settings[key])}
                  class="focus-visible:ring-ring/50 inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors focus-visible:ring-3 outline-none {!!settings[key] ? 'bg-primary' : 'bg-input'}">
                  <span class="pointer-events-none block size-4 rounded-full bg-white shadow-sm transition-transform {!!settings[key] ? 'translate-x-4' : 'translate-x-0'}"></span>
                </button>
                <span class="text-sm">{!!settings[key] ? 'Enabled' : 'Disabled'}</span>
              </div>
            {:else if p.type === "integer" || p.type === "number"}
              <Input id={"s-" + key} type="number" min={p.minimum} max={p.maximum} step={p.type === "integer" ? 1 : 0.01}
                value={settings[key] ?? ""} oninput={(e) => updateField(key, parseNum((e.target as HTMLInputElement).value, p.type!))} />
            {:else if p.type === "string" && p.enum}
              <select id={"s-" + key} value={settings[key] as string ?? ""} onchange={(e) => updateField(key, (e.target as HTMLSelectElement).value)}
                class="dark:bg-input/30 border-input focus-visible:border-ring focus-visible:ring-ring/50 h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm outline-none">
                {#each p.enum as opt}<option value={opt}>{opt}</option>{/each}
              </select>
            {:else if p.format === "uri" || p.format === "url"}
              <Input id={"s-" + key} type="url" placeholder="https://" value={settings[key] as string ?? ""}
                oninput={(e) => updateField(key, (e.target as HTMLInputElement).value)} />
            {:else}
              <Input id={"s-" + key} type="text" value={settings[key] as string ?? ""}
                oninput={(e) => updateField(key, (e.target as HTMLInputElement).value)} />
            {/if}
          </div>
          {#if i < Object.entries(schema.properties ?? {}).length - 1}<Separator class="my-1" />{/if}
        {/each}
      </div>
      <div class="flex items-center gap-2 pt-2">
        <Button type="submit" disabled={saving}>
          {#if saving}<RefreshCwIcon class="size-4 animate-spin" />{:else}<SaveIcon class="size-4" />{/if}
          Save Settings
        </Button>
        <Button variant="outline" type="button" onclick={loadSettings} disabled={saving}>
          <RefreshCwIcon class="size-4" /> Refresh
        </Button>
      </div>
    </form>
  {/if}
</div>
