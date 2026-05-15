<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { ArrowLeftIcon, GlobeIcon } from "@lucide/svelte";
  import { isAuthenticated, isConfigured } from "$lib/api";

  const id = $derived($page.params.id);
  onMount(() => { if (!isConfigured() || !isAuthenticated()) goto("/"); });
</script>

<div class="mx-auto max-w-2xl px-6 py-6">
  <button onclick={() => goto(`/servers/${id}`)} class="mb-4 text-sm text-muted-foreground hover:text-foreground"><ArrowLeftIcon class="inline size-4" /> Back</button>
  <h1 class="mb-2 text-xl font-semibold">World Management</h1>
  <div class="rounded-lg border bg-card p-8 text-center text-muted-foreground">
    <GlobeIcon class="mx-auto mb-2 size-10" />
    <p class="text-sm">World management requires backend API endpoints.</p>
    <p class="text-xs">See <code class="rounded bg-muted px-1">api_needed.md</code></p>
  </div>
</div>
