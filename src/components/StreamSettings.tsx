import {
  Popover,
  PopoverBackdrop,
  PopoverButton,
  PopoverPanel,
} from "@headlessui/react";
import type { StreamPrefs } from "../types";
import { DROPDOWN_TRIGGER_OPEN, DROPDOWN_WIDTH } from "./dropdown";
import { PixelIcon } from "./PixelIcon";

type Props = {
  prefs: StreamPrefs;
  disabled: boolean;
  onChange: (prefs: StreamPrefs) => void;
};

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

export function StreamSettings({ prefs, disabled, onChange }: Props) {
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
              <div>
                <Switch
                  label="Mouse"
                  value={prefs.gameMouse ? "game" : "desktop"}
                  disabled={disabled}
                  options={[
                    { value: "desktop", label: "Desktop" },
                    { value: "game", label: "Game" },
                  ]}
                  onChange={(mode) => set({ gameMouse: mode === "game" })}
                />
                <p className="mt-2 text-moonlight/70">
                  Desktop follows your cursor and stops at the window edge.
                  Game sends raw movement so aiming never stops; click the
                  window to capture the mouse.
                </p>
              </div>
              <div>
                <Switch
                  label="System keys → host"
                  value={prefs.systemKeys}
                  disabled={disabled}
                  options={[
                    { value: "never", label: "Off" },
                    { value: "fullscreen", label: "Fullscreen" },
                    { value: "always", label: "Always" },
                  ]}
                  onChange={(systemKeys) => set({ systemKeys })}
                />
                <p className="mt-2 text-moonlight/70">
                  Alt+Tab, Super / Windows, and Alt+F4 go to the host instead
                  of this PC. Ctrl+Alt+K flips it for one window.
                </p>
              </div>
              <Switch
                label="Play audio on"
                value={prefs.audioOnHost ? "host" : "client"}
                disabled={disabled}
                options={[
                  { value: "client", label: "This PC" },
                  { value: "host", label: "Host" },
                ]}
                onChange={(where) => set({ audioOnHost: where === "host" })}
              />
              <Switch
                label="Audio channels"
                value={prefs.audioChannels}
                disabled={disabled || prefs.audioOnHost}
                options={[
                  { value: "stereo", label: "Stereo" },
                  { value: "5.1-surround", label: "5.1" },
                  { value: "7.1-surround", label: "7.1" },
                ]}
                onChange={(audioChannels) => set({ audioChannels })}
              />
            </div>
          </PopoverPanel>
        </>
      )}
    </Popover>
  );
}
