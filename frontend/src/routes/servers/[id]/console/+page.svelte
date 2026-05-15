<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  const id = $derived($page.params.id);
  onMount(() => goto(`/servers/${id}/logs`, { replaceState: true }));
</script>
