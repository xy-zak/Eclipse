export type SidecarStatus = {
  sunshinePresent: boolean;
  moonlightPresent: boolean;
  sunshineRunning: boolean;
  keepHostOnExit: boolean;
  hostStartOnBoot: boolean;
  hostServiceInstalled: boolean;
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
  tailscaleInstalled: boolean;
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
  gameMouse: boolean;
  systemKeys: SystemKeys;
  audioOnHost: boolean;
  audioChannels: AudioChannels;
  yuv444: boolean;
};

export type HostLockStatus = {
  on: boolean;
  supported: boolean;
  lastError: string | null;
};

export type SystemKeys = "never" | "fullscreen" | "always";
export type AudioChannels = "stereo" | "5.1-surround" | "7.1-surround";

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
  gameMouse: boolean;
  systemKeys: SystemKeys;
  audioOnHost: boolean;
  audioChannels: AudioChannels;
  yuv444: boolean;
  displays: StreamWindowStatus[];
  scaleOptions: ScaleOption[];
};

export type InboundStream = {
  ip: string;
};
