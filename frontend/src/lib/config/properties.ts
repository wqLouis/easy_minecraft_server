/// Server property metadata for the config editor

export interface PropConfig {
  label: string;
  description: string;
  type: "text" | "number" | "enum" | "boolean";
  options?: string[];
}

export const propConfig: Record<string, PropConfig> = {
  difficulty: {
    label: "Difficulty",
    description: "Game difficulty level",
    type: "enum",
    options: ["peaceful", "easy", "normal", "hard"],
  },
  gamemode: {
    label: "Gamemode",
    description: "Default game mode for new players",
    type: "enum",
    options: ["survival", "creative", "adventure", "spectator"],
  },
  "server-port": { label: "Server Port", description: "Port the server listens on", type: "number" },
  motd: { label: "Server Description (MOTD)", description: "Message shown in the server list", type: "text" },
  "max-players": { label: "Max Players", description: "Maximum number of players that can join", type: "number" },
  "online-mode": { label: "Online Mode", description: "Require Mojang authentication", type: "boolean" },
  pvp: { label: "PvP", description: "Allow player versus player combat", type: "boolean" },
  "enable-command-block": { label: "Command Blocks", description: "Enable command blocks in-game", type: "boolean" },
  "spawn-protection": { label: "Spawn Protection", description: "Radius of spawn protection (0 to disable)", type: "number" },
  "spawn-animals": { label: "Spawn Animals", description: "Allow animals to spawn naturally", type: "boolean" },
  "spawn-monsters": { label: "Spawn Monsters", description: "Allow monsters to spawn naturally", type: "boolean" },
  "spawn-npcs": { label: "Spawn NPCs", description: "Allow villagers to spawn naturally", type: "boolean" },
  "allow-flight": { label: "Allow Flight", description: "Allow players to fly if they have the right items", type: "boolean" },
  hardcore: { label: "Hardcore", description: "Enable hardcore mode (ban on death)", type: "boolean" },
  "white-list": { label: "Whitelist", description: "Only whitelisted players can join", type: "boolean" },
  "enforce-whitelist": { label: "Enforce Whitelist", description: "Kick players not on the whitelist", type: "boolean" },
};

export const essentialKeys = Object.keys(propConfig);
