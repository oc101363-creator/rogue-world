import { useCallback, useEffect, useMemo, useState } from "react";
import { RotMap } from "./map/RotMap";
import { SidePanel } from "./panel/SidePanel";
import type { ConnectionStatus, SnapshotMessage } from "./types";
import { createFrontendWsClient } from "./ws/client";
import "./App.css";

export default function App() {
  const [snapshot, setSnapshot] = useState<SnapshotMessage | null>(null);
  const [status, setStatus] = useState<ConnectionStatus>("connecting");
  const [statusDetail, setStatusDetail] = useState<string | undefined>();

  const client = useMemo(
    () =>
      createFrontendWsClient({
        onSnapshot: (snap) => setSnapshot(snap),
        onStatus: (s, detail) => {
          setStatus(s);
          setStatusDetail(detail);
        },
      }),
    [],
  );

  useEffect(() => {
    client.connect();
    return () => client.disconnect();
  }, [client]);

  const handleSelect = useCallback(
    (agentId: string) => {
      client.selectAgent(agentId);
    },
    [client],
  );

  return (
    <div className="app">
      <main className="app__map">
        <RotMap snapshot={snapshot} onSelect={handleSelect} />
        {!snapshot ? (
          <div className="app__placeholder">
            <p>Waiting for world snapshot…</p>
            <p className="app__placeholder-sub">
              Connect World Server at ws://127.0.0.1:8080/ws/frontend
            </p>
          </div>
        ) : null}
      </main>
      <SidePanel
        status={status}
        statusDetail={statusDetail}
        snapshot={snapshot}
      />
    </div>
  );
}
