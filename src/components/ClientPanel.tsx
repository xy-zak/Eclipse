import { Menu, MenuButton, MenuItem, MenuItems } from "@headlessui/react";
import { Monitor, Moon, Network, Settings } from "lucide-react";
import type { LocalNode } from "../types";
import { InfoRow, Panel } from "./Panel";

type Props = {
  local: LocalNode | null;
  resolution: string;
  streamFps: number;
};

function dash(value: string | number | null | undefined): string {
  if (value === null || value === undefined || value === "") {
    return "—";
  }
  return String(value);
}

export function ClientPanel({ local, resolution, streamFps }: Props) {
  const speed =
    local?.linkSpeedMbps != null ? `${local.linkSpeedMbps} Mbps` : "—";

  return (
    <Panel
      accent="moonlight"
      title={dash(local?.hostname)}
      icon={<Moon size={16} strokeWidth={2} />}
      actions={
        <Menu>
          <MenuButton className="inline-flex items-center gap-2 border border-moonlight bg-eclipse px-3 py-1.5 text-sm text-moonlight hover:bg-moonlight hover:text-eclipse">
            <Settings size={14} strokeWidth={2} />
            Settings
          </MenuButton>
          <MenuItems
            anchor="bottom end"
            className="z-20 mt-1 min-w-56 border border-moonlight bg-eclipse p-3 text-sm text-moonlight shadow-none"
          >
            <MenuItem disabled>
              <p className="text-moonlight/70">Moonlight stream</p>
            </MenuItem>
            <p className="mt-1 font-medium">1080p · H.264</p>
            <p className="mt-1 font-medium">{streamFps} fps</p>
            <p className="mt-3 text-moonlight/70">Shortcuts</p>
            <p className="mt-1 font-medium">Ctrl+Esc disconnect</p>
            <p className="mt-1 font-medium">Ctrl+Tab fullscreen</p>
            <p className="mt-1 font-medium">Ctrl+` minimise</p>
          </MenuItems>
        </Menu>
      }
    >
      <InfoRow label="Operating system" value={dash(local?.os)} />
      <InfoRow label="IP" value={dash(local?.lanIp)} />
      <InfoRow label="Tailscale IP" value={dash(local?.tailscaleIp)} />
      <InfoRow
        label="Screen"
        value={resolution}
        icon={<Monitor size={14} strokeWidth={2} />}
      />
      <InfoRow
        label="Network speed"
        value={speed}
        icon={<Network size={14} strokeWidth={2} />}
      />
    </Panel>
  );
}
