export type CellKind = "wall" | "floor";

export interface Grid {
  width: number;
  height: number;
  /** cells[y][x] */
  cells: CellKind[][];
}

export function createEmptyGrid(width: number, height: number): Grid {
  const cells: CellKind[][] = [];
  for (let y = 0; y < height; y++) {
    const row: CellKind[] = [];
    for (let x = 0; x < width; x++) {
      row.push("floor");
    }
    cells.push(row);
  }
  return { width, height, cells };
}

export function isInBounds(grid: Grid, x: number, y: number): boolean {
  return x >= 0 && y >= 0 && x < grid.width && y < grid.height;
}

export function isWalkable(grid: Grid, x: number, y: number): boolean {
  if (!isInBounds(grid, x, y)) return false;
  return grid.cells[y][x] === "floor";
}

export function getCell(grid: Grid, x: number, y: number): CellKind | null {
  if (!isInBounds(grid, x, y)) return null;
  return grid.cells[y][x];
}

/** Terrain-only glyph rows (`#` / `.`). */
export function tilesAsStrings(grid: Grid): string[] {
  return grid.cells.map((row) =>
    row.map((c) => (c === "wall" ? "#" : ".")).join(""),
  );
}
