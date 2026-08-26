import {
  Popover,
  PopoverBackdrop,
  PopoverButton,
  PopoverPanel,
} from "@headlessui/react";
import { DROPDOWN_TRIGGER_OPEN, DROPDOWN_WIDTH } from "./dropdown";
import { PixelIcon } from "./PixelIcon";

const SHORTCUTS: { keys: string; action: string }[] = [
  { keys: "Ctrl+Alt+Esc", action: "Disconnect all windows" },
  { keys: "Ctrl+Alt+F", action: "Fullscreen this window" },
  { keys: "Ctrl+Alt+-", action: "List host displays" },
  { keys: "Ctrl+Alt+1–9", action: "This window → host display N" },
  { keys: "Ctrl+Alt+0", action: "One window per host display" },
  { keys: "Ctrl+Alt+Q", action: "Close this window" },
];

export function InfoPopover() {
  return (
    <Popover className={({ open }) => (open ? "relative z-50 isolate" : "")}>
      {({ open }) => (
        <>
          <PopoverButton
            aria-label="Info"
            className={`inline-flex h-9 w-9 items-center justify-center border border-moonlight bg-eclipse text-moonlight hover:bg-moonlight hover:text-eclipse ${
              open ? DROPDOWN_TRIGGER_OPEN : ""
            }`}
          >
            <PixelIcon name="exclamation" size={16} />
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
              This PC
            </p>
            <p className="mt-2 text-moonlight/70">
              Host streaming (tray) lets other machines on your Tailscale
              network control this desktop. Pause it from the tray when you do
              not want that.
            </p>
            <p className="mt-2 text-moonlight/70">
              Host admin is the local Sunshine page (username eclipse). Copy
              the Web UI password from this panel if you need it; Connect does
              not use that password.
            </p>
            <p className="mt-4 font-medium uppercase tracking-widest text-moonlight/70">
              Host certificates
            </p>
            <p className="mt-2 text-moonlight/70">
              If Connect warns that a host certificate changed, trust it only
              after a reinstall of Eclipse on that PC. App updates do not
              rotate the certificate.
            </p>
            <p className="mt-4 font-medium uppercase tracking-widest text-moonlight/70">
              Stream shortcuts
            </p>
            <p className="mt-2 text-moonlight/70">
              While a Moonlight window is focused. Combos are not sent to the
              host.
            </p>
            <dl className="mt-3 grid grid-cols-[auto_1fr] gap-x-8 gap-y-2">
              {SHORTCUTS.map((row) => (
                <div key={row.keys} className="contents">
                  <dt className="font-medium">{row.keys}</dt>
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
