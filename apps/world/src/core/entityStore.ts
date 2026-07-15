export type EntityId = string;
export type EntityType = "agent" | "resource";

export interface Position {
  x: number;
  y: number;
}

export interface Appearance {
  glyph: string;
}

export interface Inventory {
  ore: number;
}

export interface ResourceNode {
  ore: number;
}

export interface AgentBrain {
  runtimeSessionId?: string;
}

export interface ComponentMap {
  position?: Position;
  appearance?: Appearance;
  inventory?: Inventory;
  resourceNode?: ResourceNode;
  agentBrain?: AgentBrain;
}

export interface Entity {
  id: EntityId;
  type: EntityType;
  components: ComponentMap;
}

export class EntityStore {
  private entities = new Map<EntityId, Entity>();

  add(entity: Entity): void {
    if (this.entities.has(entity.id)) {
      throw new Error(`Entity already exists: ${entity.id}`);
    }
    this.entities.set(entity.id, entity);
  }

  get(id: EntityId): Entity | undefined {
    return this.entities.get(id);
  }

  has(id: EntityId): boolean {
    return this.entities.has(id);
  }

  remove(id: EntityId): boolean {
    return this.entities.delete(id);
  }

  all(): Entity[] {
    return [...this.entities.values()];
  }

  byType(type: EntityType): Entity[] {
    return this.all().filter((e) => e.type === type);
  }

  /** Entities that have all listed component keys. */
  query(...componentKeys: Array<keyof ComponentMap>): Entity[] {
    return this.all().filter((e) =>
      componentKeys.every((k) => e.components[k] !== undefined),
    );
  }

  getPosition(id: EntityId): Position | undefined {
    return this.entities.get(id)?.components.position;
  }

  setPosition(id: EntityId, pos: Position): void {
    const e = this.entities.get(id);
    if (!e) throw new Error(`Entity not found: ${id}`);
    e.components.position = { ...pos };
  }
}
