/**
 * Map themes (trimmed set).
 * Remaps frog f_info 16-color letters (and L) to RGB.
 *
 * - Qud Viridian: Caves of Qud terminal palette
 * - Catppuccin Mocha: https://github.com/catppuccin/catppuccin
 * - None: raw/high-contrast fallback (legacy harsh map look)
 */

/** @typedef {{
 *   id: string,
 *   name: string,
 *   ui: { bg: string, hud: string, hudMuted: string, online: string, offline: string },
 *   void: string,
 *   letters: Record<string, string>,
 *   cellBg: (letter: string, ch: string) => string,
 *   entities: { agent: string, tree: string, iron: string, hut: string, monster: string, item: string, entityBg: string },
 * }} Theme */

/** @type {Theme[]} */
const THEMES = [
  {
    // Caves of Qud — Viridian terminal palette
    id: "qud-viridian",
    name: "Qud Viridian",
    ui: {
      bg: "#0f3b3a",
      hud: "#b1c9c3",
      hudMuted: "#77bfcf",
      online: "#00c420",
      offline: "#f15f22",
    },
    void: "#0f3b3a",
    letters: {
      D: "#0f3b3a",
      d: "#155352",
      s: "#155352",
      w: "#b1c9c3",
      W: "#ffffff",
      b: "#0048bd",
      B: "#0096ff",
      g: "#009403",
      G: "#00c420",
      r: "#a64a2e",
      R: "#f15f22",
      o: "#e99f10",
      y: "#cfc041",
      u: "#b154cf",
      U: "#b1c9c3",
      v: "#da5bd6",
      L: "#00c420",
      l: "#40a4b9",
    },
    cellBg(letter, ch) {
      if (ch === "~" && "bB".includes(letter)) return "#0a2a32";
      if (ch === "~" && "rRo".includes(letter)) return "#2a1810";
      if (ch === "#") return "#0c2e2d";
      if (letter === "g" || letter === "L" || letter === "G") return "#0d3328";
      if (letter === "u" || letter === "v") return "#1a2438";
      if (letter === "y" || letter === "o") return "#1a2e24";
      return "#0f3b3a";
    },
    entities: {
      agent: "#cfc041",
      tree: "#00c420",
      iron: "#77bfcf",
      hut: "#e99f10",
      monster: "#f15f22",
      item: "#da5bd6",
      entityBg: "#155352",
    },
  },
  {
    id: "catppuccin-mocha",
    name: "Catppuccin Mocha",
    ui: {
      bg: "#1e1e2e",
      hud: "#cdd6f4",
      hudMuted: "#a6adc8",
      online: "#a6e3a1",
      offline: "#f38ba8",
    },
    void: "#1e1e2e",
    letters: {
      D: "#11111b",
      d: "#313244",
      s: "#6c7086",
      w: "#cdd6f4",
      W: "#bac2de",
      b: "#89b4fa",
      B: "#89dceb",
      g: "#a6e3a1",
      G: "#94e2d5",
      r: "#f38ba8",
      R: "#eba0ac",
      o: "#fab387",
      y: "#f9e2af",
      u: "#cba6f7",
      U: "#f5e0dc",
      v: "#cba6f7",
      L: "#a6e3a1",
      l: "#94e2d5",
    },
    cellBg(letter, ch) {
      if (ch === "~" && "bB".includes(letter)) return "#181825";
      if (ch === "~" && "rRo".includes(letter)) return "#2a1a1e";
      if (ch === "#") return "#181825";
      if (letter === "g" || letter === "L") return "#1a2420";
      if (letter === "u") return "#242018";
      return "#1e1e2e";
    },
    entities: {
      agent: "#f9e2af",
      tree: "#a6e3a1",
      iron: "#89dceb",
      hut: "#fab387",
      monster: "#f38ba8",
      item: "#cba6f7",
      entityBg: "#313244",
    },
  },
  {
    // Legacy high-contrast: near-black void, pale walls — “无配色 / raw”
    id: "none",
    name: "无配色",
    ui: {
      bg: "#050805",
      hud: "#c8c8c8",
      hudMuted: "#888888",
      online: "#33ff66",
      offline: "#ff4444",
    },
    void: "#050805",
    letters: {
      D: "#000000",
      d: "#404040",
      s: "#808080",
      w: "#e8e8e8",
      W: "#ffffff",
      b: "#1e90ff",
      B: "#87cefa",
      g: "#228b22",
      G: "#90ee90",
      r: "#c41e3a",
      R: "#ff6b6b",
      o: "#ff7f00",
      y: "#ffd700",
      u: "#8b4513",
      U: "#deb887",
      v: "#c44cff",
      L: "#3cb371",
      l: "#40a4b9",
    },
    cellBg(letter, ch) {
      if (ch === "~" && "bB".includes(letter)) return "#0a1520";
      if (ch === "~" && "rRo".includes(letter)) return "#2a0a0a";
      if (ch === "#") return "#0e0e10";
      if (letter === "g" || letter === "L") return "#0a1a0a";
      if (letter === "u") return "#1a140a";
      return "#050805";
    },
    entities: {
      agent: "#ffe066",
      tree: "#2dd36f",
      iron: "#c8d6e5",
      hut: "#ff9f43",
      monster: "#ff6b6b",
      item: "#c44cff",
      entityBg: "#1a280a",
    },
  },
];

function getTheme(id) {
  return THEMES.find((t) => t.id === id) || THEMES[0];
}
