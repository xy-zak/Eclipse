import {
  Popover,
  PopoverBackdrop,
  PopoverButton,
  PopoverPanel,
} from "@headlessui/react";
import type { DisplayInfo, StreamPrefs } from "../types";
import { DROPDOWN_TRIGGER_OPEN, DROPDOWN_WIDTH } from "./dropdown";
import { PixelIcon } from "./PixelIcon";

export type StreamHostPreview = {
  hostname: string;
  displays: DisplayInfo[];
};

type Props = {
  prefs: StreamPrefs;
  hosts: StreamHostPreview[];
  disabled: boolean;
  onChange: (prefs: StreamPrefs) => void;
};

function scaledSize(
  width: number,
  height: number,
  denominator: number,
): { width: number; height: number } | null {
  const n = Math.max(denominator, 1);
  if (width === 0 || height === 0) {
    return null;
  }
  const w = Math.floor(width / n) & ~1;
  const h = Math.floor(height / n) & ~1;
  if (w < 2 || h < 2) {
    return null;
  }
  return { width: w, height: h };
}

function Switch<T extends string | number>({
  label,
  value,
  options,
  disabled,
  onChange,
}: {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  disabled: boolean;
  onChange: (value: T) => void;
}) {
  return (
    <div>
      <p className="whitespace-nowrap text-moonlight/70">{label}</p>
      <div className="mt-1 flex">
        {options.map((option, i) => {
          const selected = option.value === value;
          return (
            <button
              key={String(option.value)}
              type="button"
              disabled={disabled}
              onClick={() => onChange(option.value)}
              className={`min-w-0 flex-1 whitespace-nowrap border border-sunshine px-3 py-1 text-sm font-medium disabled:cursor-not-allowed disabled:opacity-40 ${
                i > 0 ? "-ml-px" : ""
              } ${
                selected
                  ? "bg-sunshine text-eclipse"
                  : "bg-eclipse text-sunshine hover:bg-sunshine/20"
              }`}
            >
              {option.label}
            </button>
          );
        })}
      </div>
    </div>
  );
}

function ScalePreview({
  hosts,
  scale,
}: {
  hosts: StreamHostPreview[];
  scale: number;
}) {
  const rows = hosts.flatMap((host) =>
    host.displays.flatMap((display) => {
      const next = scaledSize(display.width, display.height, scale);
      if (!next) {
        return [];
      }
      const name = display.name
        ? `${host.hostname} ${display.name}`
        : host.hostname;
      return [
        `${name} ${display.width}×${display.height} → ${next.width}×${next.height}`,
      ];
    }),
  );
  if (rows.length === 0) {
    return null;
  }
  return (
    <ul className="mt-2 space-y-1 text-moonlight/70">
      {rows.map((row) => (
        <li key={row} className="wrap-break-word">
          {row}
        </li>
      ))}
    </ul>
  );
}

export function StreamSettings({ prefs, hosts, disabled, onChange }: Props) {
  const set = (patch: Partial<StreamPrefs>) => onChange({ ...prefs, ...patch });
  const scale = prefs.scale === 0 ? 1 : prefs.scale;
  return (
    <Popover className={({ open }) => (open ? "relative z-50 isolate" : "")}>
      {({ open }) => (
        <>
          <PopoverButton
            aria-label="Settings"
            className={`inline-flex h-9 w-9 items-center justify-center border border-sunshine bg-eclipse text-sunshine hover:bg-sunshine hover:text-eclipse ${
              open ? DROPDOWN_TRIGGER_OPEN : ""
            }`}
          >
            <PixelIcon name="cog" size={16} />
          </PopoverButton>
          <PopoverBackdrop
            transition
            className="fixed inset-0 z-40 bg-black/50 transition duration-200 ease-out data-closed:opacity-0"
          />
          <PopoverPanel
            modal={false}
            anchor="bottom end"
            className={`z-50 mt-1 max-h-[min(36rem,calc(100dvh-5rem))] overflow-y-auto ${DROPDOWN_WIDTH} border border-sunshine bg-eclipse p-4 text-sm text-sunshine outline-none`}
          >
            <div className="flex flex-col gap-3">
              <Switch
                label="Codec"
                value={prefs.codec}
                disabled={disabled}
                options={[
                  { value: "H.264", label: "H.264" },
                  { value: "HEVC", label: "HEVC" },
                  { value: "AV1", label: "AV1" },
                ]}
                onChange={(codec) => set({ codec })}
              />
              <div>
                <Switch
                  label="Scale"
                  value={scale}
                  disabled={disabled}
                  options={[
                    { value: 4, label: "Quarter" },
                    { value: 2, label: "Half" },
                    { value: 1, label: "Native" },
                  ]}
                  onChange={(next) => set({ scale: next })}
                />
                <p className="mt-2 text-moonlight/70">
                  Encoded size relative to the host monitor. The host display
                  mode does not change.
                </p>
                <ScalePreview hosts={hosts} scale={scale} />
              </div>
              <Switch
                label="FPS"
                value={prefs.fps}
                disabled={disabled}
                options={[
                  { value: 30, label: "30" },
                  { value: 60, label: "60" },
                ]}
                onChange={(fps) => set({ fps })}
              />
              <Switch
                label="Max bandwidth · Mb/s"
                value={prefs.bitrateMbps}
                disabled={disabled}
                options={[
                  { value: 10, label: "10" },
                  { value: 20, label: "20" },
                  { value: 50, label: "50" },
                ]}
                onChange={(bitrateMbps) => set({ bitrateMbps })}
              />
            </div>
          </PopoverPanel>
        </>
      )}
    </Popover>
  );
}
