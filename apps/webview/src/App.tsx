import { createEnvelope, type HostMessage } from "@harmony/protocol";
import { HarmonyWebSocketClient, type ConnectionState, resolveBootstrap } from "@harmony/webview-bridge";
import { useEffect, useMemo, useRef, useState } from "react";
import { ConnectionStatus } from "./components/ConnectionStatus";

export default function App() {
  const bootstrap = useMemo(() => resolveBootstrap(window), []);
  const clientRef = useRef<HarmonyWebSocketClient | null>(null);
  const [state, setState] = useState<ConnectionState>("idle");
  const [lastMessage, setLastMessage] = useState<HostMessage | null>(null);

  useEffect(() => {
    const client = new HarmonyWebSocketClient(bootstrap);
    clientRef.current = client;

    const offStatus = client.onStatus(setState);
    const offMessage = client.onMessage((message) => {
      setLastMessage(message);
    });

    client.connect();

    return () => {
      offStatus();
      offMessage();
      client.dispose();
    };
  }, [bootstrap]);

  const sendPing = () => {
    clientRef.current?.send(
      createEnvelope("ping", {
        source: bootstrap.host,
        note: "hello from shared webview"
      })
    );
  };

  return (
    <main className="app-shell">
      <header>
        <p className="kicker">Harmony</p>
        <h1>Shared Webview Host Shell</h1>
        <p>
          One React app running in Tauri, VSCode, and IntelliJ, with a common websocket bridge.
        </p>
      </header>

      <ConnectionStatus
        host={bootstrap.host}
        wsUrl={bootstrap.wsUrl}
        state={state}
        lastMessageType={lastMessage?.type}
      />

      <section className="panel">
        <button type="button" onClick={sendPing}>
          Send ping
        </button>
        <pre>{lastMessage ? JSON.stringify(lastMessage, null, 2) : "No messages yet."}</pre>
      </section>
    </main>
  );
}
