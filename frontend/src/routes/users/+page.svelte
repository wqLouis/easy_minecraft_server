<script lang="ts">
  import { onMount } from "svelte";
  import { toast } from "svelte-sonner";
  import { UsersIcon, PlusIcon, RefreshCwIcon, Trash2Icon, PencilIcon, XIcon, CheckIcon, KeyIcon, AlertCircleIcon } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { isAuthenticated, getApi } from "$lib/api";
  import type { UserProfile } from "$lib/types";

  let users = $state<UserProfile[]>([]);
  let loading = $state(true), error = $state("");
  let showCreate = $state(false), newUsername = $state(""), creating = $state(false), createdKey = $state("");
  let editingId = $state<string | null>(null), editUsername = $state("");

  onMount(() => {
    if (!isAuthenticated()) { toast.error("Auth required"); return }
    loadUsers();
  });

  async function loadUsers() {
    loading = true; error = "";
    try { users = await getApi().get<UserProfile[]>("/api/users") }
    catch (e: unknown) { error = e instanceof Error ? e.message : ""; toast.error("Failed", { description: error }) }
    finally { loading = false }
  }

  async function create() {
    if (!newUsername.trim()) return;
    creating = true; createdKey = "";
    try {
      const res = await getApi().post<{ api_key: string; user: UserProfile }>("/api/auth/register", { username: newUsername.trim() });
      createdKey = res.api_key;
      users = await getApi().get<UserProfile[]>("/api/users");
      toast.success("User created");
    } catch (e: unknown) { toast.error("Failed", { description: e instanceof Error ? e.message : "" }) }
    finally { creating = false }
  }

  function resetCreate() { showCreate = false; newUsername = ""; createdKey = "" }

  async function del(id: string) {
    const u = users.find((u) => u.id === id);
    if (!u || !confirm(`Delete "${u.username}"?`)) return;
    try { await getApi().del(`/api/users/${id}`); users = users.filter((u) => u.id !== id); toast.success("User deleted") }
    catch (e: unknown) { toast.error("Failed", { description: e instanceof Error ? e.message : "" }) }
  }

  function startEdit(u: UserProfile) { editingId = u.id; editUsername = u.username }
  function cancelEdit() { editingId = null }
  async function saveEdit(id: string) {
    if (!editUsername.trim()) return;
    try { await getApi().put(`/api/users/${id}`, { username: editUsername.trim() }); users = users.map((u) => u.id === id ? { ...u, username: editUsername.trim() } : u); editingId = null; toast.success("Updated") }
    catch (e: unknown) { toast.error("Failed", { description: e instanceof Error ? e.message : "" }) }
  }
</script>

<div class="mx-auto max-w-3xl px-6 py-6">
  <div class="mb-6 flex items-center justify-between">
    <div>
      <div class="flex items-center gap-2"><UsersIcon class="size-5" /><h1 class="text-xl font-semibold">Users</h1></div>
      <p class="text-sm text-muted-foreground">Manage registered users.</p>
    </div>
    <div class="flex items-center gap-2">
      <Button variant="outline" size="sm" onclick={loadUsers} disabled={loading}><RefreshCwIcon class={loading ? "size-4 animate-spin" : "size-4"} /> Refresh</Button>
      <Button size="sm" onclick={() => { resetCreate(); showCreate = true }}><PlusIcon class="size-4" /> New User</Button>
    </div>
  </div>

  {#if loading}
    <div class="flex items-center justify-center py-20"><RefreshCwIcon class="size-8 animate-spin text-muted-foreground" /></div>
  {:else if error}
    <div class="flex flex-col items-center gap-4 py-20 text-center"><AlertCircleIcon class="size-12" /><p class="text-sm text-muted-foreground">{error}</p><Button onclick={loadUsers}>Retry</Button></div>
  {:else}
    <Card.Root size="sm">
      <div class="overflow-x-auto">
        <table class="w-full text-sm">
          <thead>
            <tr class="border-b text-left text-xs text-muted-foreground">
              <th class="px-4 py-3 font-medium">Username</th><th class="px-4 py-3 font-medium">Role</th><th class="px-4 py-3 font-medium">Created</th><th class="px-4 py-3 font-medium text-right">Actions</th>
            </tr>
          </thead>
          <tbody>
            {#each users as user (user.id)}
              <tr class="border-b last:border-0 hover:bg-muted/30">
                <td class="px-4 py-3">
                  {#if editingId === user.id}
                    <div class="flex items-center gap-1">
                      <Input bind:value={editUsername} class="h-7 text-xs" />
                      <Button size="icon-xs" variant="ghost" onclick={() => saveEdit(user.id)}><CheckIcon class="size-3" /></Button>
                      <Button size="icon-xs" variant="ghost" onclick={cancelEdit}><XIcon class="size-3" /></Button>
                    </div>
                  {:else}<span class="font-medium">{user.username}</span>{/if}
                </td>
                <td class="px-4 py-3"><Badge variant={user.is_sudoer ? "default" : "secondary"}>{user.is_sudoer ? "Sudo" : "User"}</Badge></td>
                <td class="px-4 py-3 text-muted-foreground">{user.created_at}</td>
                <td class="px-4 py-3 text-right">
                  <div class="flex items-center justify-end gap-1">
                    <Button variant="ghost" size="icon-xs" onclick={() => startEdit(user)} disabled={editingId !== null}><PencilIcon class="size-3" /></Button>
                    <Button variant="ghost" size="icon-xs" class="text-destructive hover:text-destructive" onclick={() => del(user.id)} disabled={editingId !== null}><Trash2Icon class="size-3" /></Button>
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

<Dialog.Root bind:open={showCreate}>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>Create New User</Dialog.Title>
      <Dialog.Description>Enter a username. API key shown once.</Dialog.Description>
    </Dialog.Header>
    {#if createdKey}
      <div class="rounded-md border border-amber-300 bg-amber-50 p-4 text-sm text-amber-800 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-300">
        <div class="mb-1 flex items-center gap-2 font-medium"><KeyIcon class="size-4" /> API Key</div>
        <p class="mb-2 text-xs">Save this key — it will not be shown again.</p>
        <code class="block break-all rounded bg-amber-100 p-2 font-mono text-xs dark:bg-amber-900">{newUsername.trim()}:{createdKey}</code>
        <p class="mt-2 text-xs">Use this as <code class="rounded bg-amber-100 px-1 dark:bg-amber-900">Authorization: Bearer {newUsername.trim()}:{createdKey}</code></p>
      </div>
      <Dialog.Footer><Button onclick={resetCreate}>Close</Button></Dialog.Footer>
    {:else}
      <div class="grid gap-4">
        <div class="grid gap-2"><label for="new-username" class="text-sm font-medium">Username</label><Input id="new-username" placeholder="myuser" bind:value={newUsername} disabled={creating} /></div>
      </div>
      <Dialog.Footer>
        <Button variant="outline" onclick={resetCreate} disabled={creating}>Cancel</Button>
        <Button onclick={create} disabled={!newUsername.trim() || creating}>{creating ? "Creating…" : "Create User"}</Button>
      </Dialog.Footer>
    {/if}
  </Dialog.Content>
</Dialog.Root>
