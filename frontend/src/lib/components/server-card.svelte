<script lang="ts">
  import type { MinecraftServer } from "$lib/types";
  import * as Card from "$lib/components/ui/card/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem } from "$lib/components/ui/dropdown-menu/index.js";
  import { ServerIcon, PlayIcon, SquareIcon, Trash2Icon, MoreHorizontalIcon, TerminalIcon, ExternalLinkIcon } from "@lucide/svelte";

  let { server, onStart, onStop, onDelete, onViewLogs, onView }: {
    server: MinecraftServer;
    onStart: (id: string) => void;
    onStop: (id: string) => void;
    onDelete: (id: string) => void;
    onViewLogs: (id: string) => void;
    onView?: (id: string) => void;
  } = $props();

  function stop(e: MouseEvent) { e.stopPropagation(); if (server.running) onStop(server.id) }
  function del(e: MouseEvent) { e.stopPropagation(); onDelete(server.id) }
  function logs(e: MouseEvent) { e.stopPropagation(); onViewLogs(server.id) }
  function start(e: MouseEvent) { e.stopPropagation(); onStart(server.id) }
  function view(e: MouseEvent) { e.stopPropagation(); onView?.(server.id) }
</script>

<Card.Root size="sm" class="cursor-pointer transition-colors hover:bg-accent/20" onclick={view}>
  <Card.Header class="flex-row items-start justify-between gap-4">
    <div class="flex flex-col gap-1 min-w-0">
      <div class="flex items-center gap-2">
        <ServerIcon class="size-4 shrink-0 text-muted-foreground" />
        <Card.Title class="text-base truncate">{server.name}</Card.Title>
      </div>
      <Card.Description class="text-xs truncate">{server.jar_path ?? "No JAR"}</Card.Description>
    </div>
    <div class="flex items-center gap-2 shrink-0">
      <div class="flex items-center gap-1.5">
        <span class={"size-2 rounded-full " + (server.running ? "bg-green-500" : "bg-gray-400")}></span>
        <span class="text-xs font-medium text-muted-foreground">{server.running ? "Running" : "Stopped"}</span>
      </div>
      <DropdownMenu>
        <DropdownMenuTrigger onclick={(e: MouseEvent) => e.stopPropagation()}>
          <Button variant="ghost" size="icon-sm"><MoreHorizontalIcon class="size-4" /></Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onclick={view}><ExternalLinkIcon class="size-4" /> Details</DropdownMenuItem>
          <DropdownMenuItem onclick={logs}><TerminalIcon class="size-4" /> View Logs</DropdownMenuItem>
          <DropdownMenuItem disabled={server.running} onclick={start}><PlayIcon class="size-4" /> Start</DropdownMenuItem>
          <DropdownMenuItem disabled={!server.running} onclick={stop}><SquareIcon class="size-4" /> Stop</DropdownMenuItem>
          <DropdownMenuItem variant="destructive" onclick={del}><Trash2Icon class="size-4" /> Delete</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  </Card.Header>
  <Card.Footer class="flex items-center justify-between border-t pt-3 text-xs text-muted-foreground">
    <span class="truncate">{server.server_dir ?? ""}</span>
    <Badge variant="outline" class="flex items-center gap-1 shrink-0">
      {#if server.running}<span class="size-1.5 rounded-full bg-green-500"></span>{/if}
      {server.running ? "Online" : "Offline"}
    </Badge>
  </Card.Footer>
</Card.Root>
