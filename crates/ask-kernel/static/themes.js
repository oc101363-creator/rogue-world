/**
 * Map themes — classic + modern terminal palettes.
 * Each theme remaps frog f_info 16-color letters (and L for leafy) to RGB.
 *
 * Sources / inspiration:
 * - Nord: https://www.nordtheme.com/
 * - Catppuccin: https://github.com/catppuccin/catppuccin
 * - Gruvbox: https://github.com/morhetz/gruvbox
 * - Solarized: https://ethanschoonover.com/solarized/
 * - Dracula: https://draculatheme.com/
 * - Everforest (soft forest): common terminal/editor ports
 * - Qud Viridian: Caves of Qud terminal palette (ANSI + Solarized-ish teal)
 */

/** @typedef {{
 *   id: string,
 *   name: string,
 *   ui: { bg: string, hud: string, hudMuted: string, online: string, offline: string },
 *   void: string,
 *   letters: Record<string, string>,
 *   cellBg: (letter: string, ch: string) => string,
 *   entities: { agent: string, tree: string, iron: string, hut: string, entityBg: string },
 * }} Theme */

/** @type {Theme[]} */
const THEMES = [
  {
    id: "nord",
    name: "Nord",
    ui: {
      bg: "#2e3440",
      hud: "#d8dee9",
      hudMuted: "#81a1c1",
      online: "#a3be8c",
      offline: "#bf616a",
    },
    void: "#2e3440",
    // polar night + snow + frost + aurora mapped onto frog letters
    letters: {
      D: "#2e3440",
      d: "#3b4252",
      s: "#4c566a",
      w: "#e5e9f0",
      W: "#d8dee9",
      b: "#5e81ac",
      B: "#88c0d0",
      g: "#a3be8c",
      G: "#b48ead",
      r: "#bf616a",
      R: "#d08770",
      o: "#d08770",
      y: "#ebcb8b",
      u: "#8f7a66",
      U: "#d8dee9",
      v: "#b48ead",
      L: "#a3be8c",
      l: "#8fbcbb",
    },
    cellBg(letter, ch) {
      if ("~".includes(ch) && "bB".includes(letter)) return "#3b4252";
      if ("~".includes(ch) && "rR".includes(letter)) return "#3b2f2f";
      if ("#".includes(ch)) return "#3b4252";
      if (letter === "g" || letter === "L") return "#334038";
      if (letter === "u") return "#3b3832";
      return "#2e3440";
    },
    entities: {
      agent: "#ebcb8b",
      tree: "#a3be8c",
      iron: "#88c0d0",
      hut: "#d08770",
      entityBg: "#3b4252",
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
      entityBg: "#313244",
    },
  },
  {
    id: "gruvbox",
    name: "Gruvbox Dark",
    ui: {
      bg: "#282828",
      hud: "#ebdbb2",
      hudMuted: "#a89984",
      online: "#b8bb26",
      offline: "#fb4934",
    },
    void: "#282828",
    letters: {
      D: "#1d2021",
      d: "#3c3836",
      s: "#928374",
      w: "#ebdbb2",
      W: "#d5c4a1",
      b: "#458588",
      B: "#83a598",
      g: "#98971a",
      G: "#b8bb26",
      r: "#cc241d",
      R: "#fb4934",
      o: "#d65d0e",
      y: "#d79921",
      u: "#a89984",
      U: "#d5c4a1",
      v: "#b16286",
      L: "#b8bb26",
      l: "#8ec07c",
    },
    cellBg(letter, ch) {
      if (ch === "~" && "bB".includes(letter)) return "#1d2828";
      if (ch === "~" && "rRo".includes(letter)) return "#3a2018";
      if (ch === "#") return "#1d2021";
      if (letter === "g" || letter === "L") return "#2a2e1a";
      if (letter === "u") return "#322e24";
      return "#282828";
    },
    entities: {
      agent: "#fabd2f",
      tree: "#b8bb26",
      iron: "#83a598",
      hut: "#fe8019",
      entityBg: "#3c3836",
    },
  },
  {
    id: "solarized",
    name: "Solarized Dark",
    ui: {
      bg: "#002b36",
      hud: "#93a1a1",
      hudMuted: "#586e75",
      online: "#859900",
      offline: "#dc322f",
    },
    void: "#002b36",
    letters: {
      D: "#002b36",
      d: "#073642",
      s: "#586e75",
      w: "#93a1a1",
      W: "#eee8d5",
      b: "#268bd2",
      B: "#2aa198",
      g: "#859900",
      G: "#719e07",
      r: "#dc322f",
      R: "#cb4b16",
      o: "#cb4b16",
      y: "#b58900",
      u: "#657b83",
      U: "#93a1a1",
      v: "#6c71c4",
      L: "#859900",
      l: "#2aa198",
    },
    cellBg(letter, ch) {
      if (ch === "~" && "bB".includes(letter)) return "#073642";
      if (ch === "~" && "rRo".includes(letter)) return "#3a1a12";
      if (ch === "#") return "#073642";
      if (letter === "g" || letter === "L") return "#0a3020";
      if (letter === "u") return "#0f2a28";
      return "#002b36";
    },
    entities: {
      agent: "#b58900",
      tree: "#859900",
      iron: "#2aa198",
      hut: "#cb4b16",
      entityBg: "#073642",
    },
  },
  {
    id: "dracula",
    name: "Dracula",
    ui: {
      bg: "#282a36",
      hud: "#f8f8f2",
      hudMuted: "#6272a4",
      online: "#50fa7b",
      offline: "#ff5555",
    },
    void: "#282a36",
    letters: {
      D: "#21222c",
      d: "#44475a",
      s: "#6272a4",
      w: "#f8f8f2",
      W: "#f8f8f2",
      b: "#8be9fd",
      B: "#8be9fd",
      g: "#50fa7b",
      G: "#50fa7b",
      r: "#ff5555",
      R: "#ffb86c",
      o: "#ffb86c",
      y: "#f1fa8c",
      u: "#ff79c6",
      U: "#f8f8f2",
      v: "#bd93f9",
      L: "#50fa7b",
      l: "#8be9fd",
    },
    cellBg(letter, ch) {
      if (ch === "~" && "bB".includes(letter)) return "#1e2433";
      if (ch === "~" && "rRo".includes(letter)) return "#3a2030";
      if (ch === "#") return "#21222c";
      if (letter === "g" || letter === "L") return "#1e3028";
      if (letter === "u") return "#322430";
      return "#282a36";
    },
    entities: {
      agent: "#f1fa8c",
      tree: "#50fa7b",
      iron: "#8be9fd",
      hut: "#ffb86c",
      entityBg: "#44475a",
    },
  },
  {
    id: "everforest",
    name: "Everforest",
    ui: {
      bg: "#2d353b",
      hud: "#d3c6aa",
      hudMuted: "#859289",
      online: "#a7c080",
      offline: "#e67e80",
    },
    void: "#2d353b",
    letters: {
      D: "#232a2e",
      d: "#3d484d",
      s: "#859289",
      w: "#d3c6aa",
      W: "#d3c6aa",
      b: "#7fbbb3",
      B: "#83c092",
      g: "#a7c080",
      G: "#a7c080",
      r: "#e67e80",
      R: "#e69875",
      o: "#e69875",
      y: "#dbbc7f",
      u: "#d699b6",
      U: "#d3c6aa",
      v: "#d699b6",
      L: "#a7c080",
      l: "#83c092",
    },
    cellBg(letter, ch) {
      if (ch === "~" && "bB".includes(letter)) return "#273a3a";
      if (ch === "~" && "rRo".includes(letter)) return "#3a2c2a";
      if (ch === "#") return "#232a2e";
      if (letter === "g" || letter === "L") return "#2a3830";
      if (letter === "u") return "#35302e";
      return "#2d353b";
    },
    entities: {
      agent: "#dbbc7f",
      tree: "#a7c080",
      iron: "#7fbbb3",
      hut: "#e69875",
      entityBg: "#3d484d",
    },
  },
  {
    // Caves of Qud — Viridian terminal palette (user-provided 16-color map)
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
    // frog letters → Qud ANSI-ish colors
    letters: {
      D: "#0f3b3a", // black / bg
      d: "#155352", // bright black
      s: "#155352",
      w: "#b1c9c3", // white normal (fungal grey text)
      W: "#ffffff", // bright white
      b: "#0048bd", // deep sea blue
      B: "#0096ff", // cyber sky blue
      g: "#009403", // jungle green
      G: "#00c420", // neon green
      r: "#a64a2e", // rust red
      R: "#f15f22", // fire orange-red
      o: "#e99f10", // amber / orange
      y: "#cfc041", // bright lemon
      u: "#b154cf", // radiation purple (brown slot → magenta family)
      U: "#b1c9c3",
      v: "#da5bd6", // mutant pink
      L: "#00c420", // leafy / int green
      l: "#40a4b9", // wetland cyan
    },
    cellBg(letter, ch) {
      if (ch === "~" && "bB".includes(letter)) return "#0a2a32"; // water
      if (ch === "~" && "rRo".includes(letter)) return "#2a1810"; // lava
      if (ch === "#") return "#0c2e2d"; // walls slightly deeper teal
      if (letter === "g" || letter === "L" || letter === "G") return "#0d3328";
      if (letter === "u" || letter === "v") return "#1a2438";
      if (letter === "y" || letter === "o") return "#1a2e24";
      return "#0f3b3a";
    },
    entities: {
      agent: "#cfc041", // bright yellow — scannable
      tree: "#00c420",
      iron: "#77bfcf",
      hut: "#e99f10",
      entityBg: "#155352",
    },
  },
];

function getTheme(id) {
  return THEMES.find((t) => t.id === id) || THEMES[0];
}
