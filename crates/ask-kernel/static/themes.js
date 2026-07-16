/**
 * FS-HDG material themes.
 * Primary path: material id → CSS. Letter maps kept only as legacy fallback.
 */

/** @typedef {{
 *  id: string,
 *  name: string,
 *  ui: { bg: string, hud: string, hudMuted: string, online: string, offline: string, accent: string },
 *  void: string,
 *  materials: Record<string, string>,
 *  memoryFactor: number,
 *  selection: string,
 *  letters?: Record<string, string>,
 *  cellBg?: Function,
 *  entities?: object,
 * }} Theme */

const FS_HDG_BASE_MATERIALS = {
  basalt: "#555555",
  granite: "#AAAAAA",
  gold: "#FFD700",
  aquifer: "#0055FF",
  magma: "#FF4500",
  organic: "#8B5A2B",
  void: "#000000",
  ui_primary: "#00FF66",
  ui_warning: "#FFCC00",
  ui_danger: "#FF3333",
  ui_info: "#00E5FF",
  text_white: "#FFFFFF",
  depth_shadow: "#2A2A2A",
};

const THEMES = [
  {
    id: "fs-hdg",
    name: "FS-HDG",
    ui: {
      bg: "#000000",
      hud: "#FFFFFF",
      hudMuted: "#00FF66",
      online: "#00FF66",
      offline: "#FF3333",
      accent: "#00E5FF",
    },
    void: "#000000",
    materials: Object.assign({}, FS_HDG_BASE_MATERIALS),
    memoryFactor: 0.4,
    selection: "#003333",
    letters: {
      D: "#000000",
      d: "#1e1e1e",
      s: "#808080",
      w: "#ffffff",
      W: "#ffffff",
      b: "#0055FF",
      B: "#00E5FF",
      g: "#00FF66",
      G: "#00FF66",
      r: "#FF3333",
      R: "#FF4500",
      o: "#FFCC00",
      y: "#FFD700",
      u: "#8B5A2B",
      U: "#AAAAAA",
      v: "#00E5FF",
      L: "#00FF66",
      l: "#00E5FF",
    },
    cellBg: function () {
      return "#000000";
    },
    entities: {
      agent: "#FFCC00",
      tree: "#8B5A2B",
      iron: "#AAAAAA",
      hut: "#FFCC00",
      monster: "#FF3333",
      item: "#00E5FF",
      entityBg: "#000000",
    },
  },
  {
    id: "catppuccin-mocha",
    name: "Catppuccin Mocha",
    ui: {
      bg: "#1e1e2e",
      hud: "#cdd6f4",
      hudMuted: "#a6e3a1",
      online: "#a6e3a1",
      offline: "#f38ba8",
      accent: "#89b4fa",
    },
    void: "#1e1e2e",
    materials: {
      basalt: "#45475a",
      granite: "#bac2de",
      gold: "#f9e2af",
      aquifer: "#89b4fa",
      magma: "#fab387",
      organic: "#a6e3a1",
      void: "#11111b",
      ui_primary: "#a6e3a1",
      ui_warning: "#f9e2af",
      ui_danger: "#f38ba8",
      ui_info: "#89dceb",
      text_white: "#cdd6f4",
      depth_shadow: "#313244",
    },
    memoryFactor: 0.45,
    selection: "#313244",
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
    cellBg: function () {
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
    // Legacy high-contrast raw look — “无配色”
    id: "none",
    name: "无配色",
    ui: {
      bg: "#050805",
      hud: "#c8c8c8",
      hudMuted: "#888888",
      online: "#33ff66",
      offline: "#ff4444",
      accent: "#40a4b9",
    },
    void: "#050805",
    materials: {
      basalt: "#e8e8e8",
      granite: "#c0c0c0",
      gold: "#ffd700",
      aquifer: "#1e90ff",
      magma: "#ff7f00",
      organic: "#228b22",
      void: "#050805",
      ui_primary: "#33ff66",
      ui_warning: "#ffd700",
      ui_danger: "#ff6b6b",
      ui_info: "#40a4b9",
      text_white: "#c8c8c8",
      depth_shadow: "#1a280a",
    },
    memoryFactor: 0.38,
    selection: "#1a280a",
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
    cellBg: function (letter, ch) {
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
  return THEMES.find(function (t) {
    return t.id === id;
  }) || THEMES[0];
}
