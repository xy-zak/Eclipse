import type { ReactNode } from "react";
import {
  Popover,
  PopoverBackdrop,
  PopoverButton,
  PopoverPanel,
} from "@headlessui/react";
import { DROPDOWN_TRIGGER_OPEN, DROPDOWN_WIDTH } from "./dropdown";
import { PixelIcon, type PixelIconName } from "./PixelIcon";

export type MenuAccent = "moonlight" | "sunshine" | "tailscale";

const CHROME: Record<MenuAccent, string> = {
  moonlight:
    "border-moonlight bg-eclipse text-moonlight hover:bg-moonlight hover:text-eclipse",
  sunshine:
    "border-sunshine bg-eclipse text-sunshine hover:bg-sunshine hover:text-eclipse",
  tailscale:
    "border-tailscale bg-eclipse text-tailscale hover:bg-tailscale hover:text-eclipse",
};

const PANEL: Record<MenuAccent, string> = {
  moonlight: "border-moonlight text-moonlight",
  sunshine: "border-sunshine text-sunshine",
  tailscale: "border-tailscale text-tailscale",
};

export function InfoMenu({
  accent,
  icon = "info-circle",
  label,
  children,
}: {
  accent: MenuAccent;
  icon?: PixelIconName;
  label: string;
  children: ReactNode;
}) {
  return (
    <Popover className={({ open }) => (open ? "relative z-50 isolate" : "")}>
      {({ open }) => (
        <>
          <PopoverButton
            aria-label={label}
            className={`inline-flex h-9 w-9 items-center justify-center border ${CHROME[accent]} ${
              open ? DROPDOWN_TRIGGER_OPEN : ""
            }`}
          >
            <PixelIcon name={icon} size={16} />
          </PopoverButton>
          <PopoverBackdrop
            transition
            className="fixed inset-0 z-40 bg-black/50 transition duration-200 ease-out data-closed:opacity-0"
          />
          <PopoverPanel
            modal={false}
            anchor="bottom end"
            className={`z-50 mt-1 max-h-[min(36rem,calc(100dvh-5rem))] overflow-y-auto ${DROPDOWN_WIDTH} border bg-eclipse p-4 text-left text-sm outline-none ${PANEL[accent]}`}
          >
            <div className="flex flex-col gap-4">{children}</div>
          </PopoverPanel>
        </>
      )}
    </Popover>
  );
}

export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div>
      <p className="font-medium uppercase tracking-widest text-moonlight/70">
        {title}
      </p>
      <p className="mt-2 text-moonlight/70">{children}</p>
    </div>
  );
}
