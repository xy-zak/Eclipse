import type { StreamPrefs } from "../types";
import { InfoMenu, Section } from "./InfoPopover";
import { StatusMark } from "./StatusMark";

type Props = {
  moonlightPresent: boolean;
  moonlightActive: boolean;
  prefs: StreamPrefs;
  onChange: (prefs: StreamPrefs) => void;
};

const STREAM_UI_OPTIONS = [
  { value: false, label: "Keyboard only" },
  { value: true, label: "Keyboard + mouse" },
] as const;

export function MoonlightPopover({
  moonlightPresent,
  moonlightActive,
  prefs,
  onChange,
}: Props) {
  return (
    <InfoMenu accent="moonlight" icon="moon-solid" label="Moonlight">
      <Section title="This PC’s Moonlight">
        Moonlight is the viewer on this machine. Connect on a host card opens
        one or more Moonlight windows on that desktop. Eclipse vendors this
        binary; do not install a separate Moonlight from your OS.
      </Section>
      <div className="flex flex-col gap-2">
        <StatusMark
          ok={moonlightPresent}
          label={
            moonlightPresent
              ? "Moonlight binary is present"
              : "Moonlight binary is missing"
          }
        />
        <StatusMark
          ok={moonlightActive}
          label={
            moonlightActive ? "Moonlight is active" : "Moonlight is inactive"
          }
        />
      </div>
      {!moonlightPresent ? (
        <p className="text-moonlight/70">
          Rebuild sidecars or reinstall Eclipse. Connect stays disabled until
          the viewer binary is present.
        </p>
      ) : null}
      <Section title="In-stream controls">
        Keyboard + mouse adds a small Eclipse logo to every stream window.
        Click it for a menu of the shortcut actions (fullscreen, mouse mode,
        displays, stats, disconnect); drag it anywhere so it stays out of the
        way. It hides while Game mouse has the pointer captured. Changes apply
        to open windows immediately.
      </Section>
      <div className="flex">
        {STREAM_UI_OPTIONS.map((option, i) => {
          const selected = option.value === prefs.streamUi;
          return (
            <button
              key={option.label}
              type="button"
              onClick={() => onChange({ ...prefs, streamUi: option.value })}
              className={`min-w-0 flex-1 whitespace-nowrap border border-moonlight px-3 py-1 text-sm font-medium ${
                i > 0 ? "-ml-px" : ""
              } ${
                selected
                  ? "bg-moonlight text-eclipse"
                  : "bg-eclipse text-moonlight hover:bg-moonlight/20"
              }`}
            >
              {option.label}
            </button>
          );
        })}
      </div>
      <Section title="How viewing works">
        Moonlight starts only when you Connect. It decodes with this PC’s GPU
        and paints each stream in its own window. Stream quality (codec, scale,
        fps, bandwidth) is under the HOSTS cog; in-stream keys are under the
        command icon.
      </Section>
      <Section title="When you cannot Connect">
        Connect is disabled while this PC is already streaming to a client.
        Force-disconnect that client from the CLIENT card first. You also need
        Tailscale online on both PCs and Sunshine running on the host.
      </Section>
    </InfoMenu>
  );
}
