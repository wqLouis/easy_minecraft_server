<script lang="ts">
	import type { MinecraftServer } from "$lib/types";
	import * as Card from "$lib/components/ui/card/index.js";
	import { Badge } from "$lib/components/ui/badge/index.js";
	import { Button } from "$lib/components/ui/button/index.js";
	import {
		DropdownMenu,
		DropdownMenuTrigger,
		DropdownMenuContent,
		DropdownMenuItem,
	} from "$lib/components/ui/dropdown-menu/index.js";
	import {
		ServerIcon,
		PlayIcon,
		SquareIcon,
		Trash2Icon,
		MoreHorizontalIcon,
		TerminalIcon,
	} from "@lucide/svelte";

	let {
		server,
		onStart,
		onStop,
		onDelete,
		onViewLogs,
	}: {
		server: MinecraftServer;
		onStart: (id: string) => void;
		onStop: (id: string) => void;
		onDelete: (id: string) => void;
		onViewLogs: (id: string) => void;
	} = $props();

	function handleStart(e: MouseEvent) {
		e.stopPropagation();
		onStart(server.id);
	}

	function handleStop(e: MouseEvent) {
		e.stopPropagation();
		if (server.running) onStop(server.id);
	}

	function handleDelete(e: MouseEvent) {
		e.stopPropagation();
		onDelete(server.id);
	}

	function handleViewLogs(e: MouseEvent) {
		e.stopPropagation();
		onViewLogs(server.id);
	}
</script>

<Card.Root size="sm" class="relative">
	<Card.Header class="flex-row items-start justify-between gap-4">
		<div class="flex flex-col gap-1">
			<div class="flex items-center gap-2">
				<ServerIcon class="size-4 text-muted-foreground" />
				<Card.Title class="text-base">{server.name}</Card.Title>
			</div>
			<Card.Description class="flex items-center gap-2 text-xs">
				{server.jar_path ?? "No JAR configured"}
			</Card.Description>
		</div>
		<div class="flex items-center gap-2">
			<div class="flex items-center gap-1.5">
				<span
					class={["size-2 rounded-full", server.running ? "bg-green-500" : "bg-gray-400"].join(" ")}
				></span>
				<span class="text-xs font-medium text-muted-foreground">
					{server.running ? "Running" : "Stopped"}
				</span>
			</div>
			<DropdownMenu>
				<DropdownMenuTrigger>
					<Button variant="ghost" size="icon-sm">
						<MoreHorizontalIcon class="size-4" />
					</Button>
				</DropdownMenuTrigger>
				<DropdownMenuContent align="end">
					<DropdownMenuItem onclick={handleViewLogs}>
						<TerminalIcon class="size-4" />
						View Logs
					</DropdownMenuItem>
					<DropdownMenuItem disabled={server.running} onclick={handleStart}>
						<PlayIcon class="size-4" />
						Start
					</DropdownMenuItem>
					<DropdownMenuItem disabled={!server.running} onclick={handleStop}>
						<SquareIcon class="size-4" />
						Stop
					</DropdownMenuItem>
					<DropdownMenuItem variant="destructive" onclick={handleDelete}>
						<Trash2Icon class="size-4" />
						Delete
					</DropdownMenuItem>
				</DropdownMenuContent>
			</DropdownMenu>
		</div>
	</Card.Header>
	<Card.Footer class="flex items-center justify-between border-t pt-3 text-xs text-muted-foreground">
		{server.server_dir ?? ""}
		<div class="flex items-center gap-1">
			{#if server.running}
				<Badge variant="outline" class="flex items-center gap-1">
					<span class="size-1.5 rounded-full bg-green-500"></span>
					Online
				</Badge>
			{:else}
				<Badge variant="outline">Offline</Badge>
			{/if}
		</div>
	</Card.Footer>
</Card.Root>
