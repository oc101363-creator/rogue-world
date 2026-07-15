export declare const PROTOCOL_VERSION: "1.0";
export type ActionType = "move" | "harvest" | "idle" | "say";
export interface MovePayload {
    dx: -1 | 0 | 1;
    dy: -1 | 0 | 1;
}
export interface HarvestPayload {
}
export interface IdlePayload {
}
export interface SayPayload {
    text: string;
}
export type Action = {
    type: "move";
    payload: MovePayload;
} | {
    type: "harvest";
    payload: HarvestPayload;
} | {
    type: "idle";
    payload: IdlePayload;
} | {
    type: "say";
    payload: SayPayload;
};
export interface ActionBatchMessage {
    type: "action_batch";
    id?: string;
    agentId: string;
    tick: number;
    actions: Action[];
}
export interface WorldEvent {
    type: string;
    payload: Record<string, unknown>;
    tick?: number;
}
export interface ObservationMessage {
    type: "observation";
    protocolVersion: typeof PROTOCOL_VERSION;
    tick: number;
    self: {
        id: string;
        x: number;
        y: number;
        inventory: {
            ore: number;
        };
    };
    visible: {
        width: number;
        height: number;
        tiles: string[];
        entities: Array<{
            id: string;
            type: "agent" | "resource";
            x: number;
            y: number;
            ore?: number;
            glyph: string;
        }>;
    };
    events: WorldEvent[];
    allowed_actions: ActionType[];
    focused: boolean;
    goal: string | null;
}
export interface SnapshotMessage {
    type: "snapshot";
    tick: number;
    width: number;
    height: number;
    tiles: string[];
    entities: Array<{
        id: string;
        type: "agent" | "resource";
        x: number;
        y: number;
        glyph: string;
        ore?: number;
        inventory?: {
            ore: number;
        };
    }>;
    focusedAgentId: string | null;
    recentEvents: WorldEvent[];
}
export interface SelectAgentMessage {
    type: "select_agent";
    agentId: string;
}
export interface HelloWorldToAgent {
    type: "hello";
    protocolVersion: typeof PROTOCOL_VERSION;
    agentId: string;
    map: {
        width: number;
        height: number;
    };
}
export interface HelloAck {
    type: "hello_ack";
    protocolVersion: typeof PROTOCOL_VERSION;
    runtime: "mock" | "llm";
}
export declare const ALLOWED_ACTIONS: ActionType[];
