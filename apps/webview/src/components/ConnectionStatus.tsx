import type { ConnectionState } from "@harmony/webview-bridge";

interface ConnectionStatusProps {
  host: string;
  wsUrl: string;
  state: ConnectionState;
  lastMessageType?: string;
}

const LABEL_BY_STATE: Record<ConnectionState, string> = {
  idle: "Idle",
  connecting: "Connecting",
  open: "Connected",
  closed: "Closed",
  error: "Error"
};

export function ConnectionStatus(props: ConnectionStatusProps) {
  const { host, wsUrl, state, lastMessageType } = props;

  return (
    <section className="status-card" aria-live="polite">
      <p className="kicker">Bridge</p>
      <h2>{LABEL_BY_STATE[state]}</h2>
      <dl>
        <div>
          <dt>Host</dt>
          <dd>{host}</dd>
        </div>
        <div>
          <dt>Socket</dt>
          <dd>{wsUrl}</dd>
        </div>
        <div>
          <dt>Last message</dt>
          <dd>{lastMessageType ?? "none"}</dd>
        </div>
      </dl>
    </section>
  );
}
