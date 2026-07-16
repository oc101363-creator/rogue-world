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
];

function getTheme(id) {
  return THEMES.find(function (t) {
    return t.id === id;
  }) || THEMES[0];
}
