<script lang="ts">
	import type { CreateServerRequest } from "$lib/types";
	import * as Dialog from "$lib/components/ui/dialog/index.js";
	import { Button } from "$lib/components/ui/button/index.js";
	import { Input } from "$lib/components/ui/input/index.js";

	let {
		open = $bindable(false),
		onSubmit,
	}: {
		open: boolean;
		onSubmit: (data: CreateServerRequest) => void;
	} = $props();

	let serverId = $state("");
	let name = $state("");
	let jarPath = $state("/srv/minecraft/paper-1.21.4.jar");
	let javaPath = $state("/usr/bin/java");
	let minMemory = $state("512M");
	let maxMemory = $state("4G");
	let serverDir = $state("/srv/minecraft");
	let jvmArgs = $state("-XX:+UseG1GC");
	let submitting = $state(false);

	function resetForm() {
		serverId = "";
		name = "";
		jarPath = "/srv/minecraft/paper-1.21.4.jar";
		javaPath = "/usr/bin/java";
		minMemory = "512M";
		maxMemory = "4G";
		serverDir = "/srv/minecraft";
		jvmArgs = "-XX:+UseG1GC";
		submitting = false;
	}

	function handleOpenChange() {
		if (!open) resetForm();
	}

	async function handleSubmit() {
		if (!name.trim()) return;
		submitting = true;
		onSubmit({
			id: serverId.trim() || name.trim().toLowerCase().replace(/\s+/g, "-"),
			name: name.trim(),
			jar_path: jarPath.trim(),
			java_path: javaPath.trim(),
			min_memory: minMemory,
			max_memory: maxMemory,
			server_dir: serverDir.trim(),
			jvm_args: jvmArgs.trim() ? jvmArgs.split(" ").filter(Boolean) : undefined,
		});
	}
</script>

<Dialog.Root bind:open onOpenChange={handleOpenChange}>
	<Dialog.Content class="sm:max-w-lg">
		<Dialog.Header>
			<Dialog.Title>Create New Server</Dialog.Title>
			<Dialog.Description>
				Configure a new Minecraft server instance.
			</Dialog.Description>
		</Dialog.Header>

		<div class="grid gap-4">
			<div class="grid gap-2">
				<label for="server-id" class="text-sm font-medium">Server ID</label>
				<Input id="server-id" placeholder="my-server" bind:value={serverId} disabled={submitting} />
			</div>

			<div class="grid gap-2">
				<label for="server-name" class="text-sm font-medium">Server Name</label>
				<Input id="server-name" placeholder="My Survival World" bind:value={name} disabled={submitting} />
			</div>

			<div class="grid gap-2">
				<label for="jar-path" class="text-sm font-medium">Server JAR Path</label>
				<Input id="jar-path" placeholder="/srv/minecraft/server.jar" bind:value={jarPath} disabled={submitting} />
			</div>

			<div class="grid gap-2">
				<label for="java-path" class="text-sm font-medium">Java Binary</label>
				<Input id="java-path" placeholder="/usr/bin/java" bind:value={javaPath} disabled={submitting} />
			</div>

			<div class="grid grid-cols-2 gap-4">
				<div class="grid gap-2">
					<label for="min-memory" class="text-sm font-medium">Min Memory</label>
					<Input id="min-memory" placeholder="512M" bind:value={minMemory} disabled={submitting} />
				</div>
				<div class="grid gap-2">
					<label for="max-memory" class="text-sm font-medium">Max Memory</label>
					<Input id="max-memory" placeholder="4G" bind:value={maxMemory} disabled={submitting} />
				</div>
			</div>

			<div class="grid gap-2">
				<label for="server-dir" class="text-sm font-medium">Server Directory</label>
				<Input id="server-dir" placeholder="/srv/minecraft" bind:value={serverDir} disabled={submitting} />
			</div>

			<div class="grid gap-2">
				<label for="jvm-args" class="text-sm font-medium">JVM Arguments (space-separated)</label>
				<Input id="jvm-args" placeholder="-XX:+UseG1GC" bind:value={jvmArgs} disabled={submitting} />
			</div>
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (open = false)} disabled={submitting}>
				Cancel
			</Button>
			<Button onclick={handleSubmit} disabled={!name.trim() || submitting}>
				{submitting ? "Creating…" : "Create Server"}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
