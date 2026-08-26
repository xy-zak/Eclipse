import {
  Popover,
  PopoverBackdrop,
  PopoverButton,
  PopoverPanel,
} from "@headlessui/react";
import type { StreamPrefs } from "../types";
import { DROPDOWN_TRIGGER_OPEN, DROPDOWN_WIDTH } from "./dropdown";
import { PixelIcon } from "./PixelIcon";

const SHORTCUTS: { key: string; action: string }[] = [
  { key: "Esc", action: "Disconnect all windows" },
  { key: "F", action: "Fullscreen this window" },
  { key: "K", action: "Super / Windows key: client or host" },
  { key: "-", action: "List host displays" },
  { key: "1–9", action: "This window → host display N" },
  { key: "0", action: "One window per host display" },
  { key: "Q", action: "Close this window" },
];

function chordPrefix(shift: boolean): string {
  return shift ? "Ctrl+Alt+Shift+" : "Ctrl+Alt+";
}

type Props = {
  prefs: StreamPrefs;
  onChange: (prefs: StreamPrefs) => void;
};

export function CommandsPopover({ prefs, onChange }: Props) {
  const shift = prefs.hotkeyShift;
  const prefix = chordPrefix(shift);
  return (
    <Popover className={({ open }) => (open ? "relative z-50 isolate" : "")}>
      {({ open }) => (
        <>
          <PopoverButton
            aria-label="Commands"
            className={`inline-flex h-9 w-9 items-center justify-center border border-moonlight bg-eclipse text-moonlight hover:bg-moonlight hover:text-eclipse ${
              open ? DROPDOWN_TRIGGER_OPEN : ""
            }`}
          >
            <PixelIcon name="command" size={16} />
          </PopoverButton>
          <PopoverBackdrop
            transition
            className="fixed inset-0 z-40 bg-black/50 transition duration-200 ease-out data-closed:opacity-0"
          />
          <PopoverPanel
            modal={false}
            anchor="bottom end"
            className={`z-50 mt-1 max-h-[min(36rem,calc(100dvh-5rem))] overflow-y-auto ${DROPDOWN_WIDTH} border border-moonlight bg-eclipse p-4 text-left text-sm text-moonlight outline-none`}
          >
            <p className="font-medium uppercase tracking-widest text-moonlight/70">
              Shortcut keys
            </p>
            <div className="mt-1 flex">
              {(
                [
                  { value: false, label: "Ctrl+Alt" },
                  { value: true, label: "Ctrl+Alt+Shift" },
                ] as const
              ).map((option, i) => {
                const selected = option.value === shift;
                return (
                  <button
                    key={option.label}
                    type="button"
                    onClick={() =>
                      onChange({ ...prefs, hotkeyShift: option.value })
                    }
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
            <p className="mt-3 text-moonlight/70">
              While a Moonlight window is focused. Combos are not sent to the
              host. Ctrl+Alt leaves Ctrl+Alt+Shift+F1–F12 free for Sunshine’s
              host display switch.
            </p>
            <dl className="mt-3 grid grid-cols-[auto_1fr] gap-x-8 gap-y-2">
              {SHORTCUTS.map((row) => (
                <div key={row.key} className="contents">
                  <dt className="font-medium">
                    {prefix}
                    {row.key}
                  </dt>
                  <dd className="text-right text-moonlight/70">{row.action}</dd>
                </div>
              ))}
            </dl>
          </PopoverPanel>
        </>
      )}
    </Popover>
  );
}
