import { HarmonyWebSocketClient, type ConnectionState, resolveBootstrap } from "@harmony/webview-bridge";
import { useEffect, useMemo, useState } from "react";
import { ConnectionStatus } from "./components/ConnectionStatus";

export default function App() {
  const bootstrap = useMemo(() => resolveBootstrap(window), []);
  const [state, setState] = useState<ConnectionState>("idle");
  const [lastMessageType, setLastMessageType] = useState<string>();

  useEffect(() => {
    const client = new HarmonyWebSocketClient(bootstrap);

    const offStatus = client.onStatus(setState);
    const offMessage = client.onMessage((message) => {
      setLastMessageType(message.type);
    });

    client.connect();

    return () => {
      offStatus();
      offMessage();
      client.dispose();
    };
  }, [bootstrap]);

  return (
    <main className="app-shell">
      <header>
        <p className="kicker">Harmony</p>
        <h1>Dev Helper</h1>
        <p>Shared webview shell is connected and ready for formal feature modules.</p>
      </header>

      <ConnectionStatus
        host={bootstrap.host}
        wsUrl={bootstrap.wsUrl}
        state={state}
        lastMessageType={lastMessageType}
      />

      <section className="panel">
        <h2>Workspace</h2>
        <p>No feature panels are loaded yet.</p>
      </section>
    </main>
  );
}
