import { useRef, useState } from "react";
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
  sunshinePresent: boolean;
  inbound: ClientLink | null;
  controlling: ClientLink | null;
  onForceDisconnect: () => void;
  onOpenHostAdmin: () => void;
  onCopyWebUiPassword: () => Promise<void>;
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

export function ClientPanel({
  local,
  tailscaleOnline,
  eclipseOnline,
  sunshinePresent,
  inbound,
  controlling,
  onForceDisconnect,
  onOpenHostAdmin,
  onCopyWebUiPassword,
}: Props) {
  const [copied, setCopied] = useState(false);
  const copyTimer = useRef<number | null>(null);

  const copyPassword = () => {
    void onCopyWebUiPassword()
      .then(() => {
        setCopied(true);
        if (copyTimer.current != null) {
          window.clearTimeout(copyTimer.current);
        }
        copyTimer.current = window.setTimeout(() => setCopied(false), 2000);
      })
      .catch(() => {
        setCopied(false);
      });
  };

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
        <p className="mt-3 text-sm leading-relaxed text-moonlight/70">
          {eclipseOnline
            ? "Machines on your Tailscale network can control this desktop while Host streaming is on."
            : "Host streaming is off. Turn it on from the tray to let this PC be a host."}
        </p>
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
      {sunshinePresent ? (
        <div className="mt-4 flex flex-wrap gap-2 border-t border-moonlight pt-4">
          <Button
            variant="ghost"
            disabled={!eclipseOnline}
            onClick={onOpenHostAdmin}
          >
            Open host admin
          </Button>
          <Button variant="ghost" onClick={copyPassword}>
            {copied ? "Copied" : "Copy Web UI password"}
          </Button>
        </div>
      ) : null}
    </Panel>
  );
}
