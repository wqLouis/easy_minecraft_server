/// Filter options for the Modrinth browser

export const typeOpts = [
  { value: "", label: "All types" },
  { value: "mod", label: "Mod" },
  { value: "plugin", label: "Plugin" },
  { value: "datapack", label: "Datapack" },
  { value: "modpack", label: "Modpack" },
  { value: "shader", label: "Shader" },
];

export const loaderOpts = [
  { value: "", label: "Any loader" },
  { value: "fabric", label: "Fabric" },
  { value: "forge", label: "Forge" },
  { value: "neoforge", label: "NeoForge" },
  { value: "quilt", label: "Quilt" },
  { value: "paper", label: "Paper" },
  { value: "purpur", label: "Purpur" },
  { value: "spigot", label: "Spigot" },
  { value: "waterfall", label: "Waterfall" },
  { value: "velocity", label: "Velocity" },
];

export const sortOpts = [
  { value: "relevance", label: "Relevance" },
  { value: "downloads", label: "Downloads" },
  { value: "follows", label: "Follows" },
  { value: "newest", label: "Newest" },
  { value: "updated", label: "Updated" },
];
