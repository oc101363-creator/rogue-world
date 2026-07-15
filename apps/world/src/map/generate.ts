import { config } from "../config.js";
import { createEmptyGrid, type Grid } from "./grid.js";

export interface GeneratedMap {
  grid: Grid;
  agent: { id: string; x: number; y: number };
  resources: Array<{ id: string; x: number; y: number; ore: number }>;
}

/**
 * Border walls + inner floors.
 * Agent at (2, height-3); three resource nodes at fixed positions.
 */
export function generateMap(
  width = config.mapWidth,
  height = config.mapHeight,
): GeneratedMap {
  const grid = createEmptyGrid(width, height);

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (x === 0 || y === 0 || x === width - 1 || y === height - 1) {
        grid.cells[y][x] = "wall";
      }
    }
  }

  const start = config.agentStart(height);
  const agent = {
    id: config.agentId,
    x: start.x,
    y: start.y,
  };

  const resources = config.resourcePositions.map((pos, i) => ({
    id: `ore-${i + 1}`,
    x: pos.x,
    y: pos.y,
    ore: config.orePerNode,
  }));

  return { grid, agent, resources };
}
