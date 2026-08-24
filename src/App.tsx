import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type SidecarStatus = {
  sunshinePresent: boolean;
  moonlightPresent: boolean;
  sunshineRunning: boolean;
  hostTriple: string;
};

type Peer = {
  hostname: string;
  ip: string;
  hostOnline: boolean;
  tailscaleOnline: boolean;
};

type DiscoverySnapshot = {
  peers: Peer[];
  tailscaleOk: boolean;
  message: string | null;
};

function pill(ok: boolean, label: string) {
  return (
    <span
      className={`rounded-full px-2.5 py-1 text-xs font-medium ${
        ok ? "bg-emerald-500/15 text-emerald-300" : "bg-zinc-700/80 text-zinc-300"
      }`}
    >
      {label}
    </span>
  );
}

function App() {
  const [status, setStatus] = useState<SidecarStatus | null>(null);
  const [discovery, setDiscovery] = useState<DiscoverySnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [connectingIp, setConnectingIp] = useState<string | null>(null);

  const refreshPeers = useCallback(() => {
    setLoading(true);
    invoke<DiscoverySnapshot>("list_peers")
      .then(setDiscovery)
      .catch((err: unknown) => setError(String(err)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    invoke<SidecarStatus>("sidecar_status")
      .then(setStatus)
      .catch((err: unknown) => setError(String(err)));
    refreshPeers();
  }, [refreshPeers]);

  const connect = (ip: string) => {
    setError(null);
    setConnectingIp(ip);
    invoke("connect", { ip })
      .catch((err: unknown) => setError(String(err)))
      .finally(() => setConnectingIp(null));
  };

  const peers = discovery?.peers ?? [];
  const moonlightReady = Boolean(status?.moonlightPresent);

  return (
    <main className="mx-auto flex min-h-full max-w-3xl flex-col gap-8 px-8 py-10">
      <header className="flex items-end justify-between gap-4">
        <div>
          <p className="text-sm uppercase tracking-[0.2em] text-sky-300/80">Control plane</p>
          <h1 className="mt-1 text-3xl font-semibold">Eclipse</h1>
        </div>
        <div className="flex flex-wrap justify-end gap-2">
          {pill(Boolean(status?.sunshinePresent), "Sunshine sidecar")}
          {pill(Boolean(status?.moonlightPresent), "Moonlight sidecar")}
          {pill(Boolean(status?.sunshineRunning), "Host process")}
        </div>
      </header>

      <section className="rounded-2xl border border-white/10 bg-white/5 p-5">
        <div className="mb-4 flex items-center justify-between gap-3">
          <h2 className="text-lg font-medium">Peers</h2>
          <button
            type="button"
            onClick={refreshPeers}
            disabled={loading}
            className="rounded-lg border border-white/15 px-3 py-2 text-sm text-zinc-200 hover:bg-white/5 disabled:opacity-50"
          >
            {loading ? "Refreshing…" : "Refresh"}
          </button>
        </div>

        {!discovery?.tailscaleOk && discovery?.message ? (
          <p className="text-sm text-amber-200/90">{discovery.message}</p>
        ) : null}

        {discovery?.tailscaleOk && peers.length === 0 ? (
          <p className="text-sm text-zinc-400">
            No other machines on this tailnet. Log another PC into the same Tailscale
            account to see it here.
          </p>
        ) : null}

        {peers.length > 0 ? (
          <ul className="divide-y divide-white/10">
            {peers.map((peer) => {
              const canConnect =
                peer.hostOnline && moonlightReady && connectingIp === null;
              const connecting = connectingIp === peer.ip;
              return (
                <li
                  key={`${peer.hostname}-${peer.ip}`}
                  className="flex items-center justify-between gap-3 py-3 first:pt-0 last:pb-0"
                >
                  <div>
                    <p className="font-medium">{peer.hostname}</p>
                    <p className="font-mono text-xs text-zinc-500">{peer.ip}</p>
                  </div>
                  <div className="flex flex-wrap items-center justify-end gap-2">
                    {pill(peer.tailscaleOnline, peer.tailscaleOnline ? "On tailnet" : "Offline")}
                    {pill(peer.hostOnline, peer.hostOnline ? "Eclipse host" : "No Sunshine")}
                    <button
                      type="button"
                      disabled={!canConnect}
                      onClick={() => connect(peer.ip)}
                      className="rounded-lg bg-sky-500 px-3 py-1.5 text-sm font-medium text-white hover:bg-sky-400 disabled:bg-sky-500/30 disabled:text-sky-100/70"
                    >
                      {connecting ? "Connecting…" : "Connect"}
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
        ) : null}

        {error ? <p className="mt-3 text-sm text-rose-300">{error}</p> : null}
        {status ? (
          <p className="mt-4 font-mono text-xs text-zinc-500">
            host triple: {status.hostTriple}
          </p>
        ) : null}
      </section>
    </main>
  );
}

export default App;
