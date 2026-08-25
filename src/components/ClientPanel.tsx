import type { DisplayInfo, LocalNode } from "../types";
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
  eclipseOnline: boolean;
  inbound: ClientLink | null;
  controlling: ClientLink | null;
  onForceDisconnect: () => void;
};

function dash(value: string | number | null | undefined): string {
  if (value === null || value === undefined || value === "") {
    return "—";
  }
  return String(value);
}

function formatDisplays(displays: DisplayInfo[] | undefined): string {
  if (!displays?.length) {
    return "—";
  }
  return displays
    .map((d) =>
      d.name ? `${d.name} ${d.width}×${d.height}` : `${d.width}×${d.height}`,
    )
    .join(", ");
}

function peerId(link: ClientLink): string {
  return link.hostname ? `${link.hostname} @ ${link.ip}` : link.ip;
}

function statusText(
  inbound: ClientLink | null,
  controlling: ClientLink | null,
): string {
  if (inbound) {
    return `Currently streaming to Client ${peerId(inbound)}`;
  }
  if (controlling) {
    return `Currently controlling Host ${peerId(controlling)}`;
  }
  return "Idle";
}

export function ClientPanel({
  local,
  tailscaleOnline,
  eclipseOnline,
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
      <InfoRow label="LAN IP" value={dash(local?.lanIp)} />
      <InfoRow label="Tailscale IP" value={dash(local?.tailscaleIp)} />
      <InfoRow label="Displays" value={formatDisplays(local?.displays)} />
      <div className="mt-4 border-t border-moonlight pt-4">
        <div className="flex flex-wrap gap-x-6 gap-y-2">
          <StatusMark
            ok={tailscaleOnline}
            label="Tailscale"
            icon={<PixelIcon name="chart-network" size={14} />}
          />
          <StatusMark
            ok={eclipseOnline}
            label="Eclipse"
            icon={<PixelIcon name="sparkles-solid" size={14} />}
          />
        </div>
        <div className="mt-3 flex min-w-0 items-start justify-between gap-4 py-1.5 text-sm">
          <span className="shrink-0 text-moonlight/70">Status</span>
          <div className="flex min-w-0 flex-col items-end gap-2">
            <span className="min-w-0 text-right font-medium wrap-break-word">
              {statusText(inbound, controlling)}
            </span>
            {inbound ? (
              <Button variant="fail" onClick={onForceDisconnect}>
                <PixelIcon name="times-circle" size={14} />
                Force disconnect from streaming client
              </Button>
            ) : null}
          </div>
        </div>
      </div>
    </Panel>
  );
}
