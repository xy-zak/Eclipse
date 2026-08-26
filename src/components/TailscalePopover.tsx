import { openUrl } from "@tauri-apps/plugin-opener";
import { InfoMenu, Section } from "./InfoPopover";
import { PixelIcon } from "./PixelIcon";
import { StatusMark } from "./StatusMark";

const TAILSCALE_DOWNLOADS = "https://tailscale.com/download";

type Props = {
  tailscaleOk: boolean;
  message: string | null;
};

export function TailscalePopover({ tailscaleOk, message }: Props) {
  const openDownloads = () => {
    void openUrl(TAILSCALE_DOWNLOADS).catch(() => {
      window.open(TAILSCALE_DOWNLOADS, "_blank", "noopener");
    });
  };

  return (
    <InfoMenu accent="tailscale" icon="chart-network" label="Tailscale">
      <Section title="Eclipse and Tailscale">
        Eclipse treats Tailscale as the LAN. It does not bundle Tailscale, log
        you in, or replace the Tailscale app. Install Tailscale on every PC and
        run <span className="text-tailscale">tailscale up</span> once so the
        node stays on the tailnet after reboot.
      </Section>
      <div>
        <StatusMark
          ok={tailscaleOk}
          label={tailscaleOk ? "Tailscale online" : "Tailscale not ready"}
          icon={<PixelIcon name="chart-network" size={14} />}
          tone="tailscale"
        />
        {message ? <p className="mt-2 text-moonlight/70">{message}</p> : null}
      </div>
      <Section title="What Tailscale manages">
        Identity, WireGuard tunnels, NAT traversal, and the CGNAT addresses
        (100.64.0.0/10). Eclipse lists peers with{" "}
        <span className="text-tailscale">tailscale status</span> and streams
        only to those IPs. GameStream ports are firewalled to the tailnet;
        tighten further with Tailscale ACLs if the tailnet has untrusted nodes.
      </Section>
      <Section title="What Eclipse manages">
        Sunshine (host) and Moonlight (viewer), pairing, and stream settings.
        Closing Eclipse does not log you out of Tailscale. Without Tailscale,
        HOSTS shows an error instead of other PCs.
      </Section>
      <button
        type="button"
        onClick={openDownloads}
        className="w-full border border-tailscale bg-eclipse px-3 py-1.5 text-sm font-medium text-tailscale hover:bg-tailscale hover:text-eclipse"
      >
        Open Tailscale downloads
      </button>
    </InfoMenu>
  );
}
