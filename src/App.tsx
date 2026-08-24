import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { currentMonitor } from "@tauri-apps/api/window";
import { Eclipse, Loader2, Moon, RefreshCw, Sun, Wifi } from "lucide-react";
import { Button } from "./components/Button";
import { ClientPanel } from "./components/ClientPanel";
import { HostPanel } from "./components/HostPanel";
import { StatusMark } from "./components/StatusMark";
import { TitleBar } from "./components/TitleBar";
import type {
  DiscoverySnapshot,
  LocalNode,
  SessionStatus,
  SidecarStatus,
} from "./types";

function fallbackResolution(): string {
  if (typeof window === "undefined") {
    return "—";
  }
  const dpr = window.devicePixelRatio || 1;
  return `${Math.round(window.screen.width * dpr)}×${Math.round(window.screen.height * dpr)}`;
}

async function clientResolution(local: LocalNode | null): Promise<string> {
  if (local?.displayWidth && local?.displayHeight) {
    return `${local.displayWidth}×${local.displayHeight}`;
  }
  try {
    const mon = await currentMonitor();
    if (mon?.size.width && mon.size.height) {
      return `${mon.size.width}×${mon.size.height}`;
    }
  } catch {
    // Permission or runtime without a window; use CSS pixels × DPR.
  }
  return fallbackResolution();
}

function formatMbps(mbps: number | null): string {
  if (mbps === null || !Number.isFinite(mbps)) {
    return "—";
  }
  if (mbps < 0.1) {
    return `${(mbps * 1000).toFixed(0)} kb/s`;
  }
  return `${mbps.toFixed(1)} Mb/s`;
}

export default function App() {
  const [status, setStatus] = useState<SidecarStatus | null>(null);
  const [discovery, setDiscovery] = useState<DiscoverySnapshot | null>(null);
  const [session, setSession] = useState<SessionStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [connectingIp, setConnectingIp] = useState<string | null>(null);
  const [latencyMs, setLatencyMs] = useState<number | null>(null);
  const [throughputMbps, setThroughputMbps] = useState<number | null>(null);
  const [resolution, setResolution] = useState(fallbackResolution);
  const traffic = useRef<{ t: number; bytes: number } | null>(null);
  const sessionRef = useRef(session);
  sessionRef.current = session;

  const refreshPeers = useCallback((showLoading = false) => {
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
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    invoke<SidecarStatus>("sidecar_status")
      .then(setStatus)
      .catch((err: unknown) => setError(String(err)));
    refreshPeers(true);
  }, [refreshPeers]);

  useEffect(() => {
    const update = () => {
      void clientResolution(discovery?.local ?? null).then(setResolution);
    };
    update();
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, [discovery?.local]);

  useEffect(() => {
    const id = window.setInterval(() => {
      invoke<SidecarStatus>("sidecar_status").then(setStatus).catch(() => {});
      invoke<SessionStatus>("session_status")
        .then(setSession)
        .catch(() => {});
    }, 1000);
    return () => window.clearInterval(id);
  }, []);

  useEffect(() => {
    const id = window.setInterval(() => refreshPeers(false), 3000);
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

  const connect = (ip: string) => {
    setError(null);
    setConnectingIp(ip);
    invoke("connect", { ip })
      .then(() => invoke<SessionStatus>("session_status").then(setSession))
      .catch((err: unknown) => setError(String(err)))
      .finally(() => setConnectingIp(null));
  };

  const disconnect = () => {
    setError(null);
    invoke("disconnect")
      .then(() => {
        traffic.current = null;
        setThroughputMbps(null);
        setLatencyMs(null);
        setSession({ connected: false, ip: null, width: 1920, height: 1080, fps: 60 });
      })
      .catch((err: unknown) => setError(String(err)));
  };

  const peers = discovery?.peers ?? [];
  const moonlightReady = Boolean(status?.moonlightPresent);
  const sunshineReady = Boolean(status?.sunshinePresent && status.sunshineRunning);
  const hostResolution = session
    ? `${session.width}×${session.height}`
    : "1920×1080";

  return (
    <div className="flex h-full min-h-0 flex-col bg-eclipse text-moonlight">
      <TitleBar />
      <main className="mx-auto flex w-full max-w-3xl flex-1 flex-col gap-8 overflow-y-auto px-8 py-8">
        <header className="flex flex-col items-center text-center">
          <h1 className="font-title flex items-center gap-3 text-5xl font-semibold tracking-tight">
            <Eclipse size={40} strokeWidth={1.75} />
            Eclipse
          </h1>
          <div className="mt-4 flex flex-wrap justify-center gap-x-8 gap-y-2">
            <StatusMark
              ok={sunshineReady}
              label="Sunshine"
              icon={<Sun size={16} strokeWidth={2} />}
            />
            <StatusMark
              ok={moonlightReady}
              label="Moonlight"
              icon={<Moon size={16} strokeWidth={2} />}
            />
            <StatusMark
              ok={Boolean(discovery?.local?.tailscaleOnline)}
              label="Tailscale"
              icon={<Wifi size={16} strokeWidth={2} />}
            />
          </div>
        </header>

        <section className="flex flex-col gap-4">
            <h2 className="font-title flex items-center gap-2 text-2xl font-semibold tracking-tight text-moonlight">
            <Moon size={22} strokeWidth={2} />
            Client
          </h2>
          <ClientPanel
            local={discovery?.local ?? null}
            resolution={resolution}
            streamFps={session?.fps ?? 60}
          />
        </section>

        <section className="flex flex-col gap-4">
          <div className="flex items-center justify-between gap-3">
            <h2 className="font-title flex items-center gap-2 text-2xl font-semibold tracking-tight text-sunshine">
              <Sun size={22} strokeWidth={2} />
              Hosts
            </h2>
            <Button
              variant="ghost"
              className="h-9 w-9 border-sunshine px-0 text-sunshine hover:bg-sunshine hover:text-eclipse disabled:border-sunshine disabled:text-sunshine"
              aria-label="Refresh"
              onClick={() => refreshPeers(true)}
              disabled={loading}
            >
              {loading ? (
                <Loader2 size={16} strokeWidth={2} className="animate-spin" />
              ) : (
                <RefreshCw size={16} strokeWidth={2} />
              )}
            </Button>
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
            const connected = Boolean(session?.connected && session.ip === peer.ip);
            const connecting = connectingIp === peer.ip;
            const canConnect =
              peer.hostOnline && moonlightReady && connectingIp === null && !session?.connected;
            return (
              <HostPanel
                key={`${peer.hostname}-${peer.ip}`}
                peer={peer}
                connected={connected}
                connecting={connecting}
                canConnect={canConnect}
                throughput={connected ? formatMbps(throughputMbps) : "—"}
                hostResolution={connected ? hostResolution : "—"}
                latency={
                  connected && latencyMs != null ? `${latencyMs} ms` : "—"
                }
                onConnect={() => connect(peer.ip)}
                onDisconnect={disconnect}
              />
            );
          })}

          {error ? (
            <p className="border border-fail px-4 py-3 text-sm text-fail">{error}</p>
          ) : null}
        </section>
      </main>
    </div>
  );
}
