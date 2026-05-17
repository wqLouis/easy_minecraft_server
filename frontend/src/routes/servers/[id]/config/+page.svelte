<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { ArrowLeftIcon, Settings2Icon, RefreshCwIcon, SaveIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Separator } from "$lib/components/ui/separator/index.js";
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import { isAuthenticated, isConfigured, getApi } from "$lib/api";

  let cfg = $state<Record<string, unknown>>({});
  let orig = $state("");
  let props = $state<Record<string, string>>({});
  let origProps = $state("");
  let loading = $state(true);
  let saving = $state(false);
  let tab = $state("essential");
  const id = $derived($page.params.id);
  const chg = $derived(JSON.stringify(cfg) !== orig);
  const chgProps = $derived(JSON.stringify(props) !== origProps);

  // Essential properties with typed inputs
  const propConfig: Record<string, { label: string; description: string; type: "text" | "number" | "enum" | "boolean"; options?: string[] }> = {
    "difficulty": { label: "Difficulty", description: "Game difficulty level", type: "enum", options: ["peaceful", "easy", "normal", "hard"] },
    "gamemode": { label: "Gamemode", description: "Default game mode for new players", type: "enum", options: ["survival", "creative", "adventure", "spectator"] },
    "server-port": { label: "Server Port", description: "Port the server listens on", type: "number" },
    "motd": { label: "Server Description (MOTD)", description: "Message shown in the server list", type: "text" },
    "max-players": { label: "Max Players", description: "Maximum number of players that can join", type: "number" },
    "online-mode": { label: "Online Mode", description: "Require Mojang authentication", type: "boolean" },
    "pvp": { label: "PvP", description: "Allow player versus player combat", type: "boolean" },
    "enable-command-block": { label: "Command Blocks", description: "Enable command blocks in-game", type: "boolean" },
    "spawn-protection": { label: "Spawn Protection", description: "Radius of spawn protection (0 to disable)", type: "number" },
    "spawn-animals": { label: "Spawn Animals", description: "Allow animals to spawn naturally", type: "boolean" },
    "spawn-monsters": { label: "Spawn Monsters", description: "Allow monsters to spawn naturally", type: "boolean" },
    "spawn-npcs": { label: "Spawn NPCs", description: "Allow villagers to spawn naturally", type: "boolean" },
    "allow-flight": { label: "Allow Flight", description: "Allow players to fly if they have the right items", type: "boolean" },
    "hardcore": { label: "Hardcore", description: "Enable hardcore mode (ban on death)", type: "boolean" },
    "white-list": { label: "Whitelist", description: "Only whitelisted players can join", type: "boolean" },
    "enforce-whitelist": { label: "Enforce Whitelist", description: "Kick players not on the whitelist", type: "boolean" },
  };

  const essentialKeys = [
    "difficulty", "gamemode", "server-port", "motd", "max-players",
    "online-mode", "pvp", "enable-command-block", "spawn-protection",
    "spawn-animals", "spawn-monsters", "spawn-npcs",
    "allow-flight", "hardcore", "white-list", "enforce-whitelist",
  ];

  onMount(async () => {
    if (!isConfigured() || !isAuthenticated()) { goto("/"); return; }
    try {
      const r = await getApi().get<{ config: Record<string, unknown> }>(`/api/instances/${id}`);
      cfg = { ...r.config }; orig = JSON.stringify(r.config);
      const p = await getApi().get<{ properties: Record<string, string> }>(`/api/instances/${id}/properties`).catch(() => ({ properties: {} }));
      props = p.properties ?? {}; origProps = JSON.stringify(props);
    } catch { goto("/"); } finally { loading = false; }
  });

  function set(k: string, v: unknown) { cfg = { ...cfg, [k]: v }; }
  function setProp(k: string, v: string) { props = { ...props, [k]: v }; }

  function handleBooleanToggle(key: string) {
    const current = props[key] ?? "false";
    setProp(key, current === "true" ? "false" : "true");
  }

  async function saveCfg() {
    saving = true;
    try { await getApi().put(`/api/instances/${id}/config`, cfg); orig = JSON.stringify(cfg); toast.success("Instance config saved"); }
    catch (e) { toast.error("Failed to save", { description: e instanceof Error ? e.message : "" }); }
    saving = false;
  }

  async function saveProps() {
    saving = true;
    try { await getApi().put(`/api/instances/${id}/properties`, props); origProps = JSON.stringify(props); toast.success("Properties saved"); }
    catch (e) { toast.error("Failed to save", { description: e instanceof Error ? e.message : "" }); }
    saving = false;
  }
</script>

<div class="mx-auto flex h-dvh max-w-2xl flex-col px-6 py-6">
  <button onclick={() => goto(`/servers/${id}`)} class="mb-4 flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground">
    <ArrowLeftIcon class="size-4" /> Back to Server
  </button>

  <div class="mb-6 flex items-center gap-2">
    <Settings2Icon class="size-5" />
    <div>
      <h1 class="text-xl font-semibold">Configuration</h1>
      <p class="text-sm text-muted-foreground">Server properties and instance settings.</p>
    </div>
  </div>

  {#if loading}
    <div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
  {:else}
    <Tabs.Root bind:value={tab} class="flex-1 min-h-0 flex flex-col">
      <Tabs.List class="mb-4 shrink-0">
        <Tabs.Trigger value="essential">Essential</Tabs.Trigger>
        <Tabs.Trigger value="advanced">Advanced</Tabs.Trigger>
        <Tabs.Trigger value="other">Other Properties</Tabs.Trigger>
      </Tabs.List>

      <!-- Essential Properties Tab (auto-fill height) -->
      <Tabs.Content value="essential" class="flex-1 min-h-0">
        {#if Object.keys(props).length === 0}
          <div class="rounded-lg border bg-card p-8 text-center text-sm text-muted-foreground">
            <Settings2Icon class="mx-auto mb-2 size-8" />
            <p>No server.properties found.</p>
            <p class="mt-1 italic">Start the server to generate default properties.</p>
          </div>
        {:else}
          <div class="flex h-full flex-col space-y-4">
            <div class="flex-1 min-h-0 overflow-y-auto rounded-lg border bg-card p-4 space-y-4">
              {#each essentialKeys as key}
                {@const pc = propConfig[key]}
                {#if pc && props[key] !== undefined}
                  {#if pc.type === "enum"}
                    <div class="grid gap-2">
                      <label for={"prop-" + key} class="flex items-center justify-between text-sm font-medium">
                        <span>{pc.label}</span>
                        <span class="text-xs text-muted-foreground">{pc.description}</span>
                      </label>
                      <select id={"prop-" + key} value={props[key]} onchange={(e) => setProp(key, (e.target as HTMLSelectElement).value)}
                        class="dark:bg-input/30 border-input focus-visible:border-ring focus-visible:ring-ring/50 h-9 w-full rounded-md border bg-transparent px-2.5 py-1 text-sm outline-none">
                        {#each pc.options! as opt}<option value={opt}>{opt}</option>{/each}
                      </select>
                    </div>
                  {:else if pc.type === "boolean"}
                    <div class="grid gap-2">
                      <div class="flex items-center justify-between">
                        <label for={"prop-" + key} class="text-sm font-medium">{pc.label}</label>
                        <span class="text-xs text-muted-foreground">{pc.description}</span>
                      </div>
                      <div class="flex items-center gap-3">
                        <button type="button" role="switch" aria-label={pc.label} aria-checked={props[key] === "true"} onclick={() => handleBooleanToggle(key)}
                          class="focus-visible:ring-ring/50 inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors focus-visible:ring-3 outline-none {props[key] === 'true' ? 'bg-primary' : 'bg-input'}">
                          <span class="pointer-events-none block size-4 rounded-full bg-white shadow-sm transition-transform {props[key] === 'true' ? 'translate-x-4' : 'translate-x-0'}"></span>
                        </button>
                        <span class="text-sm text-muted-foreground">{props[key] === "true" ? "Enabled" : "Disabled"}</span>
                      </div>
                    </div>
                  {:else if pc.type === "number"}
                    <div class="grid gap-2">
                      <label for={"prop-" + key} class="flex items-center justify-between text-sm font-medium">
                        <span>{pc.label}</span>
                        <span class="text-xs text-muted-foreground">{pc.description}</span>
                      </label>
                      <Input id={"prop-" + key} type="number" value={props[key]} oninput={(e) => setProp(key, (e.target as HTMLInputElement).value)} />
                    </div>
                  {:else}
                    <div class="grid gap-2">
                      <label for={"prop-" + key} class="flex items-center justify-between text-sm font-medium">
                        <span>{pc.label}</span>
                        <span class="text-xs text-muted-foreground">{pc.description}</span>
                      </label>
                      <Input id={"prop-" + key} type="text" value={props[key]} oninput={(e) => setProp(key, (e.target as HTMLInputElement).value)} />
                    </div>
                  {/if}
                  <Separator />
                {/if}
              {/each}
            </div>
            <Button onclick={saveProps} disabled={!chgProps || saving} class="w-full shrink-0">
              {#if saving}<RefreshCwIcon class="size-4 animate-spin" />{:else}<SaveIcon class="size-4" />{/if}
              {saving ? "Saving…" : "Save Configuration"}
            </Button>
          </div>
        {/if}
      </Tabs.Content>

      <!-- Advanced Tab: Instance config only -->
      <Tabs.Content value="advanced" class="flex-1 min-h-0 overflow-y-auto">
        <div class="space-y-4">
          <h3 class="text-sm font-semibold text-foreground">Instance Settings</h3>

          <div class="grid gap-2">
            <label for="cfg-name" class="text-sm font-medium">Server Name</label>
            <Input id="cfg-name" value={cfg.name as string ?? ""} oninput={(e) => set("name", (e.target as HTMLInputElement).value)} />
          </div>

          <Separator />

          <div class="grid grid-cols-2 gap-4">
            <div class="grid gap-2">
              <label for="cfg-min-mem" class="text-sm font-medium">Min Memory</label>
              <Input id="cfg-min-mem" value={cfg.min_memory as string ?? "1G"} oninput={(e) => set("min_memory", (e.target as HTMLInputElement).value)} />
            </div>
            <div class="grid gap-2">
              <label for="cfg-max-mem" class="text-sm font-medium">Max Memory</label>
              <Input id="cfg-max-mem" value={cfg.max_memory as string ?? "4G"} oninput={(e) => set("max_memory", (e.target as HTMLInputElement).value)} />
            </div>
          </div>

          <div class="grid gap-2">
            <label for="cfg-java" class="text-sm font-medium">Java Path</label>
            <Input id="cfg-java" value={cfg.java_path as string ?? ""} oninput={(e) => set("java_path", (e.target as HTMLInputElement).value)} />
          </div>

          <div class="grid gap-2">
            <label for="cfg-jvm" class="text-sm font-medium">JVM Arguments</label>
            <Input id="cfg-jvm" value={(cfg.jvm_args as string[])?.join(" ") ?? ""} oninput={(e) => set("jvm_args", (e.target as HTMLInputElement).value.split(/\s+/).filter(Boolean))} />
            <p class="text-xs text-muted-foreground">Space-separated JVM flags.</p>
          </div>

          <Button onclick={saveCfg} disabled={!chg || saving} class="w-full">
            {#if saving}<RefreshCwIcon class="size-4 animate-spin" />{:else}<SaveIcon class="size-4" />{/if}
            {saving ? "Saving…" : "Save Instance Config"}
          </Button>
        </div>
      </Tabs.Content>

      <!-- Other Properties Tab (auto-fill height) -->
      <Tabs.Content value="other" class="flex-1 min-h-0">
        {#if Object.keys(props).length === 0}
          <div class="rounded-lg border bg-card p-8 text-center text-sm text-muted-foreground">
            <Settings2Icon class="mx-auto mb-2 size-8" />
            <p>No server.properties found.</p>
            <p class="mt-1 italic">Start the server to generate default properties.</p>
          </div>
        {:else}
          <div class="flex h-full flex-col space-y-4">
            <div class="flex-1 min-h-0 overflow-y-auto rounded-lg border bg-card p-4 space-y-4">
              {#each Object.entries(props) as [key, val]}
                {#if !essentialKeys.includes(key)}
                  <div class="grid gap-2">
                    <label for={"prop-" + key} class="text-sm font-medium">{key}</label>
                    <Input id={"prop-" + key} value={val} oninput={(e) => setProp(key, (e.target as HTMLInputElement).value)} class="font-mono text-xs" />
                  </div>
                {/if}
              {/each}
            </div>
            <Button onclick={saveProps} disabled={!chgProps || saving} class="w-full shrink-0">
              {#if saving}<RefreshCwIcon class="size-4 animate-spin" />{:else}<SaveIcon class="size-4" />{/if}
              {saving ? "Saving…" : "Save Properties"}
            </Button>
          </div>
        {/if}
      </Tabs.Content>
    </Tabs.Root>
  {/if}
</div>
