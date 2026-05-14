<script lang="ts">
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";
	import { goto } from "$app/navigation";
	import {
		UsersIcon,
		PlusIcon,
		RefreshCwIcon,
		Trash2Icon,
		PencilIcon,
		XIcon,
		CheckIcon,
		KeyIcon,
		AlertCircleIcon,
		ArrowLeftIcon,
	} from "@lucide/svelte";
	import { Button } from "$lib/components/ui/button/index.js";
	import { Input } from "$lib/components/ui/input/index.js";
	import * as Card from "$lib/components/ui/card/index.js";
	import { Badge } from "$lib/components/ui/badge/index.js";
	import * as Dialog from "$lib/components/ui/dialog/index.js";
	import { isAuthenticated, getApi } from "$lib/api";
	import type { UserProfile } from "$lib/types";

	let users = $state<UserProfile[]>([]);
	let loading = $state(true);
	let error = $state("");

	// Create user dialog
	let showCreate = $state(false);
	let newEmail = $state("");
	let creating = $state(false);
	let createdKey = $state("");

	// Edit user
	let editingId = $state<string | null>(null);
	let editEmail = $state("");

	onMount(() => {
		if (!isAuthenticated()) {
			toast.error("Authentication required");
			goto("/");
			return;
		}
		loadUsers();
	});

	async function loadUsers() {
		loading = true;
		error = "";
		try {
			const api = getApi();
			users = await api.listUsers();
		} catch (e: unknown) {
			error = e instanceof Error ? e.message : "Failed to load users";
			toast.error("Failed to load users", { description: error });
		} finally {
			loading = false;
		}
	}

	async function handleCreate() {
		const email = newEmail.trim();
		if (!email) return;
		creating = true;
		createdKey = "";
		try {
			const api = getApi();
			const res = await api.registerUser(email);
			createdKey = res.api_key;
			users = await api.listUsers();
			toast.success(`User "${email}" created`);
		} catch (e: unknown) {
			toast.error("Failed to create user", {
				description: e instanceof Error ? e.message : "Unknown error",
			});
		} finally {
			creating = false;
		}
	}

	function resetCreateDialog() {
		showCreate = false;
		newEmail = "";
		createdKey = "";
	}

	async function handleDelete(id: string) {
		const user = users.find((u) => u.id === id);
		if (!user) return;
		if (!confirm(`Delete user "${user.email}"? This cannot be undone.`)) return;
		try {
			const api = getApi();
			await api.deleteUser(id);
			users = users.filter((u) => u.id !== id);
			toast.success(`User "${user.email}" deleted`);
		} catch (e: unknown) {
			toast.error("Failed to delete user", {
				description: e instanceof Error ? e.message : "Unknown error",
			});
		}
	}

	function startEdit(user: UserProfile) {
		editingId = user.id;
		editEmail = user.email;
	}

	function cancelEdit() {
		editingId = null;
	}

	async function saveEdit(id: string) {
		const email = editEmail.trim();
		if (!email) return;
		try {
			const api = getApi();
			await api.updateUser(id, { email });
			users = users.map((u) => (u.id === id ? { ...u, email } : u));
			editingId = null;
			toast.success("User updated");
		} catch (e: unknown) {
			toast.error("Failed to update user", {
				description: e instanceof Error ? e.message : "Unknown error",
			});
		}
	}
</script>

<div class="mx-auto max-w-3xl px-6 py-6">
	<div class="mb-6 flex items-center justify-between">
		<div>
			<div class="flex items-center gap-2">
				<UsersIcon class="size-5" />
				<h1 class="text-xl font-semibold">Users</h1>
			</div>
			<p class="text-sm text-muted-foreground">Manage registered users. Sudo privileges required.</p>
		</div>
		<div class="flex items-center gap-2">
			<Button variant="outline" size="sm" onclick={loadUsers} disabled={loading}>
				<RefreshCwIcon class={["size-4", loading ? "animate-spin" : ""].join(" ")} /> Refresh
			</Button>
			<Button size="sm" onclick={() => { resetCreateDialog(); showCreate = true; }}>
				<PlusIcon class="size-4" /> New User
			</Button>
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
	{:else if error}
		<div class="flex flex-col items-center justify-center gap-4 py-20 text-center">
			<AlertCircleIcon class="size-12 text-muted-foreground" />
			<div><h2 class="text-lg font-medium">Failed to Load</h2><p class="text-sm text-muted-foreground">{error}</p></div>
			<Button onclick={loadUsers}>Retry</Button>
		</div>
	{:else}
		<Card.Root size="sm">
			<div class="overflow-x-auto">
				<table class="w-full text-sm">
					<thead>
						<tr class="border-b text-left text-xs text-muted-foreground">
							<th class="px-4 py-3 font-medium">Email</th>
							<th class="px-4 py-3 font-medium">Role</th>
							<th class="px-4 py-3 font-medium">Created</th>
							<th class="px-4 py-3 font-medium text-right">Actions</th>
						</tr>
					</thead>
					<tbody>
						{#each users as user (user.id)}
							<tr class="border-b last:border-0 hover:bg-muted/30">
								<td class="px-4 py-3">
									{#if editingId === user.id}
										<div class="flex items-center gap-1">
											<Input bind:value={editEmail} class="h-7 text-xs" />
											<Button size="icon-xs" variant="ghost" onclick={() => saveEdit(user.id)} disabled={!editEmail.trim()}>
												<CheckIcon class="size-3" />
											</Button>
											<Button size="icon-xs" variant="ghost" onclick={cancelEdit}>
												<XIcon class="size-3" />
											</Button>
										</div>
									{:else}
										<span class="font-medium">{user.email}</span>
									{/if}
								</td>
								<td class="px-4 py-3">
									<Badge variant={user.is_sudoer ? "default" : "secondary"}>
										{user.is_sudoer ? "Sudo" : "User"}
									</Badge>
								</td>
								<td class="px-4 py-3 text-muted-foreground">{user.created_at}</td>
								<td class="px-4 py-3 text-right">
									<div class="flex items-center justify-end gap-1">
										<Button variant="ghost" size="icon-xs" onclick={() => startEdit(user)} disabled={editingId !== null}>
											<PencilIcon class="size-3" />
										</Button>
										<Button variant="ghost" size="icon-xs" class="text-destructive hover:text-destructive" onclick={() => handleDelete(user.id)} disabled={editingId !== null}>
											<Trash2Icon class="size-3" />
										</Button>
									</div>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		</Card.Root>
	{/if}
</div>

<!-- Create User Dialog -->
<Dialog.Root bind:open={showCreate}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>Create New User</Dialog.Title>
			<Dialog.Description>Enter the email for the new user. The API key will be shown once.</Dialog.Description>
		</Dialog.Header>

		{#if createdKey}
			<div class="rounded-md border border-amber-300 bg-amber-50 p-4 text-sm text-amber-800 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-300">
				<div class="mb-1 flex items-center gap-2 font-medium">
					<KeyIcon class="size-4" />
					API Key
				</div>
				<p class="mb-2 text-xs">Save this key — it will not be shown again.</p>
				<code class="block break-all rounded bg-amber-100 p-2 font-mono text-xs dark:bg-amber-900">{createdKey}</code>
			</div>
			<Dialog.Footer>
				<Button onclick={resetCreateDialog}>Close</Button>
			</Dialog.Footer>
		{:else}
			<div class="grid gap-4">
				<div class="grid gap-2">
					<label for="new-user-email" class="text-sm font-medium">Email</label>
					<Input id="new-user-email" type="email" placeholder="user@example.com" bind:value={newEmail} disabled={creating} />
				</div>
			</div>
			<Dialog.Footer>
				<Button variant="outline" onclick={resetCreateDialog} disabled={creating}>Cancel</Button>
				<Button onclick={handleCreate} disabled={!newEmail.trim() || creating}>
					{creating ? "Creating…" : "Create User"}
				</Button>
			</Dialog.Footer>
		{/if}
	</Dialog.Content>
</Dialog.Root>
