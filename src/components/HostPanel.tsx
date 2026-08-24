import { Eclipse, Gauge, Loader2, Plug, Sun, Timer, Unplug, Wifi } from "lucide-react";
import type { Peer } from "../types";
import { Button } from "./Button";
import { InfoRow, Panel } from "./Panel";
import { StatusMark } from "./StatusMark";

type Props = {
  peer: Peer;
  connected: boolean;
  connecting: boolean;
  canConnect: boolean;
  throughput: string;
  hostResolution: string;
  latency: string;
  onConnect: () => void;
  onDisconnect: () => void;
};

function dash(value: string | null | undefined): string {
  if (value === null || value === undefined || value === "") {
    return "—";
  }
  return value;
}

export function HostPanel({
  peer,
  connected,
  connecting,
  canConnect,
  throughput,
  hostResolution,
  latency,
  onConnect,
  onDisconnect,
}: Props) {
  return (
    <Panel
      accent="sunshine"
      title={peer.hostname}
      icon={<Sun size={16} strokeWidth={2} />}
      actions={
        connected ? (
          <Button variant="fail" onClick={onDisconnect}>
            <Unplug size={14} strokeWidth={2} />
            Disconnect
          </Button>
        ) : (
          <Button
            variant="sunshine"
            disabled={!canConnect}
            onClick={onConnect}
          >
            {connecting ? (
              <Loader2 size={14} strokeWidth={2} className="animate-spin" />
            ) : (
              <Plug size={14} strokeWidth={2} />
            )}
            {connecting ? "Connecting…" : "Connect"}
          </Button>
        )
      }
    >
      <InfoRow label="Operating system" value={dash(peer.os)} />
      <InfoRow label="Tailscale IP" value={peer.ip} />
      <div className="mt-4 flex flex-wrap gap-x-6 gap-y-2 border-t border-sunshine pt-4">
        <StatusMark
          ok={peer.tailscaleOnline}
          label="Tailscale"
          icon={<Wifi size={14} strokeWidth={2} />}
          tone="sunshine"
        />
        <StatusMark
          ok={peer.hostOnline}
          label="Eclipse"
          icon={<Eclipse size={14} strokeWidth={2} />}
          tone="sunshine"
        />
      </div>
      {connected ? (
        <div className="mt-4 border-t border-sunshine pt-3">
      <InfoRow
        label="Throughput"
        value={throughput}
        icon={<Gauge size={14} strokeWidth={2} />}
        accent="sunshine"
      />
      <InfoRow label="Host resolution" value={hostResolution} />
      <InfoRow
        label="Ping"
        value={latency}
        icon={<Timer size={14} strokeWidth={2} />}
        accent="sunshine"
      />
        </div>
      ) : null}
    </Panel>
  );
}
