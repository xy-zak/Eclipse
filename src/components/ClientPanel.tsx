import type { LocalNode } from "../types";
import { dash, formatDisplays } from "../format";
import { Button } from "./Button";
import { InfoRow, Panel } from "./Panel";
import { PixelIcon } from "./PixelIcon";
import { StatusMark } from "./StatusMark";

export type ClientLink = {
  hostname: string | null;
  ip: string;
};

type Props = {
  local: LocalNode | null;
  tailscaleOnline: boolean;
  sunshineOnline: boolean;
  moonlightOnline: boolean;
  inbound: ClientLink | null;
  controlling: ClientLink | null;
  onForceDisconnect: () => void;
};

function peerId(link: ClientLink): string {
  return link.hostname ? `${link.hostname} @ ${link.ip}` : link.ip;
}

export function ClientPanel({
  local,
  tailscaleOnline,
  sunshineOnline,
  moonlightOnline,
  inbound,
  controlling,
  onForceDisconnect,
}: Props) {
  return (
    <Panel
      accent="moonlight"
      title={dash(local?.hostname)}
      icon={<PixelIcon name="moon-solid" size={16} />}
    >
      <InfoRow label="Operating system" value={dash(local?.os)} />
      <InfoRow label="Window manager" value={dash(local?.windowManager)} />
      <InfoRow label="LAN IP" value={dash(local?.lanIp)} />
      <InfoRow label="Tailscale IP" value={dash(local?.tailscaleIp)} />
      <InfoRow label="Displays" value={formatDisplays(local?.displays)} />
      <div className="mt-4 border-t border-moonlight pt-4">
        <div className="flex flex-wrap gap-x-6 gap-y-2">
          <StatusMark
            ok={tailscaleOnline}
            label="Tailscale"
            icon={<PixelIcon name="chart-network" size={14} />}
            tone="tailscale"
          />
          <StatusMark
            ok={sunshineOnline}
            label="Sunshine"
            icon={<PixelIcon name="sun-solid" size={14} />}
            tone="sunshine"
          />
          <StatusMark
            ok={moonlightOnline}
            label="Moonlight"
            icon={<PixelIcon name="moon-solid" size={14} />}
          />
        </div>
        {!sunshineOnline ? (
          <p className="mt-3 text-sm leading-relaxed text-moonlight/70">
            Host streaming is off. Turn it on from the sun icon to let this PC
            be a host.
          </p>
        ) : null}
        {inbound ? (
          <div className="mt-4 border border-fail p-3">
            <p className="text-sm font-medium wrap-break-word">
              Streaming to {peerId(inbound)}
            </p>
            <Button
              variant="fail"
              className="mt-3 w-full"
              onClick={onForceDisconnect}
            >
              <PixelIcon name="times-circle" size={14} />
              Force disconnect
            </Button>
          </div>
        ) : (
          <div className="mt-3 flex min-w-0 items-start justify-between gap-4 py-1.5 text-sm">
            <span className="shrink-0 text-moonlight/70">Status</span>
            <span className="min-w-0 text-right font-medium wrap-break-word">
              {controlling
                ? `Currently controlling Host ${peerId(controlling)}`
                : "Idle"}
            </span>
          </div>
        )}
      </div>
    </Panel>
  );
}
