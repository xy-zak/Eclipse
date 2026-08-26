export type SidecarStatus = {
  sunshinePresent: boolean;
  moonlightPresent: boolean;
  sunshineRunning: boolean;
  keepHostOnExit: boolean;
  hostTriple: string;
};

export type DisplayInfo = {
  name: string;
  width: number;
  height: number;
  focused: boolean;
};

export type LocalNode = {
  hostname: string;
  lanIp: string | null;
  tailscaleIp: string | null;
  tailscaleOnline: boolean;
  linkSpeedMbps: number | null;
  rxBytes: number | null;
  txBytes: number | null;
  displays: DisplayInfo[];
  os: string;
  windowManager: string;
};

export type Peer = {
  hostname: string;
  ip: string;
  hostOnline: boolean;
  tailscaleOnline: boolean;
  rxBytes: number | null;
  txBytes: number | null;
  os: string;
  windowManager: string;
  displays: DisplayInfo[];
};

export type DiscoverySnapshot = {
  local: LocalNode | null;
  peers: Peer[];
  tailscaleOk: boolean;
  message: string | null;
};

export type ScaleOption = {
  denominator: number;
  width: number;
  height: number;
};

export type StreamWindowStatus = {
  displayIndex: number;
  name: string;
  nativeWidth: number;
  nativeHeight: number;
  streamWidth: number;
  streamHeight: number;
  windowOpen: boolean;
};

export type StreamPrefs = {
  scale: number;
  fps: number;
  codec: string;
  bitrateMbps: number;
  hotkeyShift: boolean;
};

export type SessionStatus = {
  connected: boolean;
  ip: string | null;
  width: number;
  height: number;
  fps: number;
  scale: number;
  codec: string;
  bitrateMbps: number;
  hotkeyShift: boolean;
  displays: StreamWindowStatus[];
  scaleOptions: ScaleOption[];
};

export type InboundStream = {
  ip: string;
};
