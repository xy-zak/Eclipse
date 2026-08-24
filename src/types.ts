export type SidecarStatus = {
  sunshinePresent: boolean;
  moonlightPresent: boolean;
  sunshineRunning: boolean;
  hostTriple: string;
};

export type LocalNode = {
  hostname: string;
  lanIp: string | null;
  tailscaleIp: string | null;
  tailscaleOnline: boolean;
  linkSpeedMbps: number | null;
  rxBytes: number | null;
  txBytes: number | null;
  displayWidth: number | null;
  displayHeight: number | null;
  os: string;
};

export type Peer = {
  hostname: string;
  ip: string;
  hostOnline: boolean;
  tailscaleOnline: boolean;
  rxBytes: number | null;
  txBytes: number | null;
  os: string;
};

export type DiscoverySnapshot = {
  local: LocalNode | null;
  peers: Peer[];
  tailscaleOk: boolean;
  message: string | null;
};

export type SessionStatus = {
  connected: boolean;
  ip: string | null;
  width: number;
  height: number;
  fps: number;
};
