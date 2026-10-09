import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "./components/Button";
import { CertTrustDialog, parseTlsCertChangedHost } from "./components/CertTrustDialog";
import { ClientPanel, type ClientLink } from "./components/ClientPanel";
import { HostPanel } from "./components/HostPanel";
import { CommandsPopover } from "./components/CommandsPopover";
import { AboutOverlay } from "./components/AboutOverlay";
import { MentalModelOverlay } from "./components/MentalModelOverlay";
import { MoonlightPopover } from "./components/MoonlightPopover";
import { PixelIcon } from "./components/PixelIcon";
import { StreamSettings } from "./components/StreamSettings";
import { SunshinePopover } from "./components/SunshinePopover";
import { TailscalePopover } from "./components/TailscalePopover";
import { TitleBar } from "./components/TitleBar";
import type {
  DiscoverySnapshot,
  HostLockStatus,
  InboundStream,
  SessionStatus,
  SidecarStatus,
  StreamPrefs,
} from "./types";

const DEFAULT_PREFS: StreamPrefs = {
  scale: 1,
  fps: 60,
  codec: "H.264",
  bitrateMbps: 20,
  hotkeyShift: false,
  gameMouse: false,
  systemKeys: "fullscreen",
  audioOnHost: false,
  audioChannels: "stereo",
  yuv444: false,
};

function formatMbps(mbps: number | null): string {
  if (mbps === null || !Number.isFinite(mbps)) {
    return "—";
  }
  if (mbps < 0.1) {
    return `${(mbps * 1000).toFixed(0)} kb/s`;
  }
  return `${mbps.toFixed(1)} Mb/s`;
}

function peerLink(
  peers: { hostname: string; ip: string }[],
  ip: string | null | undefined,
): ClientLink | null {
  if (!ip) {
    return null;
  }
  const peer = peers.find((p) => p.ip === ip);
  return { hostname: peer?.hostname ?? null, ip };
}

function prefsFromSession(session: SessionStatus | null): StreamPrefs {
  if (!session) {
    return DEFAULT_PREFS;
  }
  return {
    scale: session.scale,
    fps: session.fps,
    codec: session.codec || DEFAULT_PREFS.codec,
    bitrateMbps: session.bitrateMbps || DEFAULT_PREFS.bitrateMbps,
    hotkeyShift: Boolean(session.hotkeyShift),
    gameMouse: Boolean(session.gameMouse),
    systemKeys: session.systemKeys || DEFAULT_PREFS.systemKeys,
    audioOnHost: Boolean(session.audioOnHost),
    audioChannels: session.audioChannels || DEFAULT_PREFS.audioChannels,
    yuv444: Boolean(session.yuv444),
  };
}

export default function App() {
  const [status, setStatus] = useState<SidecarStatus | null>(null);
  const [discovery, setDiscovery] = useState<DiscoverySnapshot | null>(null);
  const [session, setSession] = useState<SessionStatus | null>(null);
  const [inbound, setInbound] = useState<InboundStream | null>(null);
  const [hostLock, setHostLock] = useState<HostLockStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [refreshPulse, setRefreshPulse] = useState(0);
  const [connectingIp, setConnectingIp] = useState<string | null>(null);
  const [guideOpen, setGuideOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [certPromptIp, setCertPromptIp] = useState<string | null>(null);
  const [latencyMs, setLatencyMs] = useState<number | null>(null);
  const [throughputMbps, setThroughputMbps] = useState<number | null>(null);
  const traffic = useRef<{ t: number; bytes: number } | null>(null);
  const peersInFlight = useRef(false);
  const statusInFlight = useRef(false);
  const sessionRef = useRef(session);
  sessionRef.current = session;

  const refreshPeers = useCallback((showLoading = false) => {
    if (peersInFlight.current) {
      return;
    }
    peersInFlight.current = true;
    if (showLoading) {
      setLoading(true);
    }
    invoke<DiscoverySnapshot>("list_peers")
      .then((snap) => {
        setDiscovery(snap);
        const current = sessionRef.current;
        const ip = current?.ip;
        if (!current?.connected || !ip) {
          traffic.current = null;
          setThroughputMbps(null);
          return;
        }
        const peer = snap.peers.find((p) => p.ip === ip);
        const bytes = (peer?.rxBytes ?? 0) + (peer?.txBytes ?? 0);
        const now = Date.now();
        const prev = traffic.current;
        if (prev && now > prev.t) {
          const mbps = ((bytes - prev.bytes) * 8) / ((now - prev.t) / 1000) / 1e6;
          if (mbps >= 0) {
            setThroughputMbps(mbps);
          }
        }
        traffic.current = { t: now, bytes };
      })
      .catch((err: unknown) => setError(String(err)))
      .finally(() => {
        peersInFlight.current = false;
        setLoading(false);
      });
  }, []);

  useEffect(() => {
    invoke<SidecarStatus>("sidecar_status")
      .then(setStatus)
      .catch((err: unknown) => setError(String(err)));
    invoke<SessionStatus>("session_status")
      .then(setSession)
      .catch(() => {});
    invoke<InboundStream | null>("inbound_status")
      .then(setInbound)
      .catch(() => setInbound(null));
    invoke<HostLockStatus>("host_lock_status")
      .then(setHostLock)
      .catch(() => {});
    refreshPeers(true);
  }, [refreshPeers]);

  useEffect(() => {
    const tick = () => {
      if (statusInFlight.current) {
        return;
      }
      statusInFlight.current = true;
      Promise.all([
        invoke<SidecarStatus>("sidecar_status").then(setStatus).catch(() => {}),
        invoke<SessionStatus>("session_status")
          .then(setSession)
          .catch(() => {}),
        invoke<InboundStream | null>("inbound_status")
          .then(setInbound)
          .catch(() => setInbound(null)),
        // Picks up a failed automatic lock (lastError) after a stream ends.
        invoke<HostLockStatus>("host_lock_status")
          .then(setHostLock)
          .catch(() => {}),
      ]).finally(() => {
        statusInFlight.current = false;
      });
    };
    const id = window.setInterval(tick, 2000);
    return () => window.clearInterval(id);
  }, []);

  useEffect(() => {
    const id = window.setInterval(() => refreshPeers(false), 5000);
    return () => window.clearInterval(id);
  }, [refreshPeers]);

  useEffect(() => {
    if (!session?.connected || !session.ip) {
      setLatencyMs(null);
      return;
    }
    const ping = () => {
      invoke<number | null>("peer_latency", { ip: session.ip })
        .then(setLatencyMs)
        .catch(() => setLatencyMs(null));
    };
    ping();
    const id = window.setInterval(ping, 2000);
    return () => window.clearInterval(id);
  }, [session?.connected, session?.ip]);

  const connect = (ip: string, trustNewCert = false) => {
    setError(null);
    setCertPromptIp(null);
    setConnectingIp(ip);
    const displays = peers.find((p) => p.ip === ip)?.displays ?? [];
    invoke("connect", { ip, trustNewCert, displays })
      .then(() => invoke<SessionStatus>("session_status").then(setSession))
      .catch((err: unknown) => {
        const changed = parseTlsCertChangedHost(err);
        if (changed) {
          setCertPromptIp(changed);
          return;
        }
        setError(String(err));
      })
      .finally(() => setConnectingIp(null));
  };

  const disconnect = () => {
    setError(null);
    invoke("disconnect")
      .then(() => {
        traffic.current = null;
        setThroughputMbps(null);
        setLatencyMs(null);
        invoke<SessionStatus>("session_status").then(setSession).catch(() => {});
      })
      .catch((err: unknown) => setError(String(err)));
  };

  const forceDisconnectInbound = () => {
    setError(null);
    invoke("force_disconnect_inbound")
      .then(() => {
        setInbound(null);
        invoke<InboundStream | null>("inbound_status")
          .then(setInbound)
          .catch(() => setInbound(null));
      })
      .catch((err: unknown) => setError(String(err)));
  };

  const openHostAdmin = () => {
    setError(null);
    invoke("open_host_admin").catch((err: unknown) => setError(String(err)));
  };

  const setHostRunning = async (on: boolean) => {
    setError(null);
    try {
      await invoke("set_host_running", { on });
      setStatus(await invoke<SidecarStatus>("sidecar_status"));
    } catch (err: unknown) {
      setError(String(err));
      throw err;
    }
  };

  const setKeepHostOnExit = async (on: boolean) => {
    setError(null);
    try {
      await invoke("set_keep_host_on_exit", { on });
      setStatus(await invoke<SidecarStatus>("sidecar_status"));
    } catch (err: unknown) {
      setError(String(err));
      throw err;
    }
  };

  const setHostStartOnBoot = async (on: boolean) => {
    setError(null);
    try {
      await invoke("set_host_start_on_boot", { on });
      setStatus(await invoke<SidecarStatus>("sidecar_status"));
    } catch (err: unknown) {
      setError(String(err));
      throw err;
    }
  };

  const setHostLockOnEnd = async (on: boolean) => {
    setError(null);
    try {
      setHostLock(await invoke<HostLockStatus>("set_host_lock_on_end", { on }));
    } catch (err: unknown) {
      setError(String(err));
      throw err;
    }
  };

  const testHostLock = async () => {
    setError(null);
    try {
      setHostLock(await invoke<HostLockStatus>("test_host_lock"));
    } catch (err: unknown) {
      setError(String(err));
      throw err;
    }
  };

  const copyWebUiPassword = async () => {
    setError(null);
    try {
      const pass = await invoke<string>("web_ui_password");
      await navigator.clipboard.writeText(pass);
    } catch (err: unknown) {
      setError(String(err));
      throw err;
    }
  };

  const setPrefs = (prefs: StreamPrefs) => {
    setError(null);
    invoke("set_stream_prefs", { prefs })
      .then(() => invoke<SessionStatus>("session_status").then(setSession))
      .catch((err: unknown) => setError(String(err)));
  };

  const peers = discovery?.peers ?? [];
  const moonlightReady = Boolean(status?.moonlightPresent);
  const sunshineReady = Boolean(status?.sunshinePresent && status.sunshineRunning);
  const streamResolution = session?.connected
    ? session.displays
        .filter((d) => d.windowOpen)
        .map((d) => {
          const scale =
            session.scale === 4
              ? " (quarter)"
              : session.scale === 2
                ? " (half)"
                : session.scale === 1
                  ? " (native)"
                  : "";
          return `${d.nativeWidth}×${d.nativeHeight} → ${d.streamWidth}×${d.streamHeight}${scale}`;
        })
        .join(", ") || `${session.width}×${session.height}`
    : "—";
  const prefs = prefsFromSession(session);
  const inboundLink = peerLink(peers, inbound?.ip);
  const controllingLink =
    session?.connected ? peerLink(peers, session.ip) : null;

  return (
    <div className="font-body flex h-full min-h-0 flex-col overflow-hidden bg-eclipse text-moonlight">
      <TitleBar />
      <main className="flex min-h-0 w-full flex-1 flex-col overflow-y-auto">
        <div className="mx-auto flex w-full min-w-0 max-w-[1080px] flex-col gap-8 px-6 py-6 sm:px-8 sm:py-8">
        <header className="flex flex-col items-center gap-3">
          <div className="grid w-full grid-cols-[1fr_auto_1fr] items-center">
            <span aria-hidden />
            <h1 className="font-title inline-flex items-center gap-3 text-7xl font-normal uppercase tracking-tight sm:gap-4 sm:text-[5.25rem]">
              <PixelIcon name="sparkles-solid" size={56} />
              Eclipse
              <PixelIcon name="sparkles-solid" size={56} className="-scale-x-100" />
            </h1>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                aria-label="How Eclipse works"
                className="inline-flex h-9 w-9 items-center justify-center border border-moonlight text-moonlight hover:bg-moonlight hover:text-eclipse"
                onClick={() => setGuideOpen(true)}
              >
                <PixelIcon name="info-circle" size={16} />
              </button>
              <button
                type="button"
                aria-label="About Eclipse"
                className="inline-flex h-9 w-9 items-center justify-center border border-moonlight text-moonlight hover:bg-moonlight hover:text-eclipse"
                onClick={() => setAboutOpen(true)}
              >
                <PixelIcon name="question-circle" size={16} />
              </button>
            </div>
          </div>
          <p className="max-w-xl text-center text-sm leading-relaxed text-moonlight/70">
            the free, open-source and cross-platform remote-desktop app
          </p>
        </header>

        <section className="flex min-w-0 flex-col gap-4">
          <div className="flex min-h-9 items-center justify-between gap-3">
            <h2 className="min-w-0 flex-1 text-sm leading-snug font-medium tracking-widest text-moonlight/70">
              CLIENT [ this computer ]
            </h2>
            <div className="flex shrink-0 items-center gap-2">
              <MoonlightPopover
                moonlightPresent={Boolean(status?.moonlightPresent)}
                moonlightActive={Boolean(session?.connected)}
              />
              <SunshinePopover
                sunshinePresent={Boolean(status?.sunshinePresent)}
                sunshineRunning={Boolean(status?.sunshineRunning)}
                keepHostOnExit={Boolean(status?.keepHostOnExit)}
                hostStartOnBoot={Boolean(status?.hostStartOnBoot)}
                hostServiceInstalled={Boolean(status?.hostServiceInstalled)}
                onOpenHostAdmin={openHostAdmin}
                onCopyWebUiPassword={copyWebUiPassword}
                onSetHostRunning={setHostRunning}
                onSetKeepHostOnExit={setKeepHostOnExit}
                onSetHostStartOnBoot={setHostStartOnBoot}
                hostLock={hostLock}
                onSetHostLockOnEnd={setHostLockOnEnd}
                onTestHostLock={testHostLock}
              />
              <TailscalePopover
                tailscaleOk={Boolean(discovery?.tailscaleOk)}
                tailscaleInstalled={Boolean(discovery?.tailscaleInstalled)}
                message={discovery?.message ?? null}
              />
              <CommandsPopover prefs={prefs} onChange={setPrefs} />
            </div>
          </div>
          <ClientPanel
            local={discovery?.local ?? null}
            tailscaleOnline={Boolean(discovery?.local?.tailscaleOnline)}
            sunshineOnline={sunshineReady}
            moonlightOnline={moonlightReady}
            inbound={inboundLink}
            controlling={controllingLink}
            onForceDisconnect={forceDisconnectInbound}
          />
        </section>

        <div aria-hidden className="h-px" />

        <section className="flex min-w-0 flex-col gap-4">
          <div className="flex min-h-9 items-center justify-between gap-3">
            <h2 className="min-w-0 flex-1 text-sm leading-snug font-medium tracking-widest text-moonlight/70">
              HOSTS [ remote-desktop target computers ]
            </h2>
            <div className="flex shrink-0 items-center gap-2">
              <StreamSettings
                prefs={prefs}
                disabled={connectingIp !== null || inbound !== null}
                onChange={setPrefs}
              />
              <Button
                variant="ghost"
                className="h-9 w-9 border-sunshine px-0 text-sunshine hover:bg-sunshine hover:text-eclipse disabled:border-sunshine disabled:text-sunshine"
                aria-label="Refresh"
                onClick={() => {
                  setRefreshPulse((n) => n + 1);
                  refreshPeers(true);
                }}
                disabled={loading}
              >
                <PixelIcon
                  name="refresh"
                  size={16}
                  className={loading ? "animate-spin" : ""}
                />
              </Button>
            </div>
          </div>

          {!discovery?.tailscaleOk && discovery?.message ? (
            <p className="border border-fail px-4 py-3 text-sm text-fail">
              {discovery.message}
            </p>
          ) : null}

          {discovery?.tailscaleOk && peers.length === 0 ? (
            <p className="border border-moonlight px-4 py-3 text-sm">
              No other machines on this tailnet. Log another PC into the same
              Tailscale account to see it here.
            </p>
          ) : null}

          {peers.map((peer) => {
            const connected = Boolean(
              session?.connected && session.ip === peer.ip,
            );
            const connecting = connectingIp === peer.ip;
            const canConnect =
              peer.hostOnline &&
              moonlightReady &&
              connectingIp === null &&
              certPromptIp === null &&
              !session?.connected &&
              inbound === null;
            return (
              <HostPanel
                key={`${peer.hostname}-${peer.ip}`}
                peer={peer}
                connected={connected}
                connecting={connecting}
                canConnect={canConnect}
                throughput={connected ? formatMbps(throughputMbps) : "—"}
                streamResolution={connected ? streamResolution : "—"}
                latency={
                  connected && latencyMs != null ? `${latencyMs} ms` : "—"
                }
                onConnect={() => connect(peer.ip)}
                onDisconnect={disconnect}
                flashKey={refreshPulse}
              />
            );
          })}

          {error ? (
            <p className="border border-fail px-4 py-3 text-sm text-fail">
              {error}
            </p>
          ) : null}
        </section>
        </div>
      </main>
      <MentalModelOverlay
        open={guideOpen}
        onClose={() => setGuideOpen(false)}
      />
      <AboutOverlay open={aboutOpen} onClose={() => setAboutOpen(false)} />
      {certPromptIp ? (
        <CertTrustDialog
          ip={certPromptIp}
          hostname={
            peers.find((peer) => peer.ip === certPromptIp)?.hostname ?? null
          }
          onCancel={() => setCertPromptIp(null)}
          onTrust={() => connect(certPromptIp, true)}
        />
      ) : null}
    </div>
  );
}
