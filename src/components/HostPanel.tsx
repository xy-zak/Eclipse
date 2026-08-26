import type { Peer } from "../types";
import { dash, formatDisplays } from "../format";
import { Button } from "./Button";
import { InfoRow, Panel } from "./Panel";
import { PixelIcon } from "./PixelIcon";
import { StatusMark } from "./StatusMark";

type Props = {
  peer: Peer;
  connected: boolean;
  connecting: boolean;
  canConnect: boolean;
  throughput: string;
  streamResolution: string;
  latency: string;
  onConnect: () => void;
  onDisconnect: () => void;
  flashKey?: number;
};

export function HostPanel({
  peer,
  connected,
  connecting,
  canConnect,
  throughput,
  streamResolution,
  latency,
  onConnect,
  onDisconnect,
  flashKey = 0,
}: Props) {
  return (
    <Panel
      accent="sunshine"
      title={peer.hostname}
      icon={<PixelIcon name="sun-solid" size={16} />}
      className="overflow-hidden"
      actions={
        connected ? (
          <Button variant="fail" onClick={onDisconnect}>
            <PixelIcon name="times-circle" size={14} />
            Disconnect
          </Button>
        ) : (
          <Button
            variant="sunshine"
            disabled={!canConnect}
            onClick={onConnect}
          >
            {connecting ? (
              <PixelIcon name="spinner" size={14} className="animate-spin" />
            ) : (
              <PixelIcon name="check-circle" size={14} />
            )}
            {connecting ? "Connecting…" : "Connect"}
          </Button>
        )
      }
    >
      {flashKey > 0 ? (
        <div
          key={flashKey}
          className="eclipse-host-flash pointer-events-none absolute inset-0 z-10 bg-sunshine"
        />
      ) : null}
      <InfoRow label="Operating system" value={dash(peer.os)} />
      <InfoRow label="Window manager" value={dash(peer.windowManager)} />
      <InfoRow label="Tailscale IP" value={peer.ip} />
      <InfoRow
        label="Displays"
        value={peer.hostOnline ? formatDisplays(peer.displays) : "—"}
      />
      <div className="mt-4 flex flex-wrap gap-x-6 gap-y-2 border-t border-sunshine pt-4">
        <StatusMark
          ok={peer.tailscaleOnline}
          label="Tailscale"
          icon={<PixelIcon name="chart-network" size={14} />}
          tone="tailscale"
        />
        <StatusMark
          ok={peer.hostOnline}
          label="Eclipse"
          icon={<PixelIcon name="sparkles-solid" size={14} />}
          tone="sunshine"
        />
      </div>
      {connected ? (
        <div className="mt-4 border-t border-sunshine pt-3">
      <InfoRow
        label="Throughput"
        value={throughput}
        icon={<PixelIcon name="analytics" size={14} />}
        accent="sunshine"
      />
      <InfoRow label="Stream" value={streamResolution} />
      <InfoRow
        label="Ping"
        value={latency}
        icon={<PixelIcon name="clock" size={14} />}
        accent="sunshine"
      />
        </div>
      ) : null}
    </Panel>
  );
}
