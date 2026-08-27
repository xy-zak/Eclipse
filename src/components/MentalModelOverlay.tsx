import { useRef, type ReactNode } from "react";
import {
  Dialog,
  DialogPanel,
  DialogTitle,
} from "@headlessui/react";
import { PixelIcon, type PixelIconName } from "./PixelIcon";
import { EclipseFooterGif } from "./EclipseFooterGif";

type Props = {
  open: boolean;
  onClose: () => void;
};

const TONE = {
  moonlight: "text-moonlight",
  sunshine: "text-sunshine",
  tailscale: "text-tailscale",
} as const;

function Named({
  icon,
  tone,
  children,
}: {
  icon: PixelIconName;
  tone: keyof typeof TONE;
  children: string;
}) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 whitespace-nowrap align-middle font-medium ${TONE[tone]}`}
    >
      <PixelIcon name={icon} size={14} />
      {children}
    </span>
  );
}

function Glyph({
  icon,
  tone,
  label,
}: {
  icon: PixelIconName;
  tone: keyof typeof TONE;
  label: string;
}) {
  return (
    <span
      className={`inline-flex items-center align-middle ${TONE[tone]}`}
      title={label}
    >
      <PixelIcon name={icon} size={16} />
      <span className="sr-only">{label}</span>
    </span>
  );
}

function Moon() {
  return <Glyph icon="moon-solid" tone="moonlight" label="Moonlight" />;
}

function Sun() {
  return <Glyph icon="sun-solid" tone="sunshine" label="Sunshine" />;
}

function Tail() {
  return <Glyph icon="chart-network" tone="tailscale" label="Tailscale" />;
}

function EclipseName() {
  return (
    <Named icon="sparkles-solid" tone="moonlight">
      Eclipse
    </Named>
  );
}

function UiLabel({ children }: { children: string }) {
  return (
    <span className="whitespace-nowrap font-medium tracking-widest text-moonlight/70">
      {children}
    </span>
  );
}

function P({ children }: { children: ReactNode }) {
  return <p className="text-sm leading-relaxed text-moonlight">{children}</p>;
}

function Chip({
  icon,
  tone,
  children,
}: {
  icon: PixelIconName;
  tone: keyof typeof TONE;
  children: string;
}) {
  const border =
    tone === "sunshine"
      ? "border-sunshine"
      : tone === "tailscale"
        ? "border-tailscale"
        : "border-moonlight";
  return (
    <span
      className={`inline-flex items-center gap-1.5 border px-2 py-1 text-sm font-medium ${border} ${TONE[tone]}`}
    >
      <PixelIcon name={icon} size={14} />
      {children}
    </span>
  );
}

export function MentalModelOverlay({ open, onClose }: Props) {
  const closeRef = useRef<HTMLButtonElement>(null);
  return (
    <Dialog
      open={open}
      onClose={onClose}
      initialFocus={closeRef}
      className="relative z-50"
    >
      <div className="fixed inset-0 flex flex-col bg-eclipse">
        <DialogPanel className="flex min-h-0 flex-1 flex-col outline-none">
          <div className="min-h-0 flex-1 overflow-y-auto">
            <div className="mx-auto flex w-full min-w-0 max-w-[1080px] flex-col px-6 sm:px-8">
              <div className="sticky top-0 z-10 flex justify-end bg-eclipse py-4">
                <button
                  ref={closeRef}
                  type="button"
                  aria-label="Back to Eclipse"
                  className="inline-flex h-9 w-9 items-center justify-center border border-moonlight text-moonlight hover:bg-moonlight hover:text-eclipse"
                  onClick={onClose}
                >
                  <PixelIcon name="times" size={16} />
                </button>
              </div>
              <div className="flex flex-col gap-8 pb-8">
                <header className="flex flex-col items-center gap-3 text-center">
                  <DialogTitle className="font-title text-4xl font-normal uppercase tracking-tight sm:text-5xl">
                    <span className="inline-flex items-center gap-3">
                      <PixelIcon name="sparkles-solid" size={28} />
                      Eclipse
                    </span>
                  </DialogTitle>
                  <p className="text-sm font-medium tracking-widest text-moonlight/70">
                    How this sky works
                  </p>
                </header>

                <div className="flex flex-col gap-5">
                  <P>
                    <EclipseName /> is the sky that holds all of your computers. Each
                    PC is a small star system: a{" "}
                    <Named icon="sun-solid" tone="sunshine">
                      Sunshine
                    </Named>{" "}
                    that shines so others can see that desktop, a{" "}
                    <Named icon="moon-solid" tone="moonlight">
                      Moonlight
                    </Named>{" "}
                    you stand on to look out, and a{" "}
                    <Named icon="chart-network" tone="tailscale">
                      Tailscale
                    </Named>{" "}
                    telescope that lets you see others’{" "}
                    <Named icon="sun-solid" tone="sunshine">
                      Sunshine
                    </Named>
                    .
                  </P>
                  <P>
                    You are always on this computer’s <Moon /> (also known as
                    the <UiLabel>CLIENT</UiLabel>). You never stand on someone
                    else’s <Moon />. You look at their <Sun />.
                  </P>
                  <P>
                    This computer also has a <Sun />. Other <Moon /> see it; you
                    tend it from the <Moon /> you are standing on. Turn it on,
                    and you are visible. Turn it off, and you go dark. It can
                    keep shining after you close the <EclipseName /> window.
                  </P>
                  <P>
                    <UiLabel>HOSTS</UiLabel> are the other <Sun /> in the sky.
                    By pressing ‘Connect’ you are aiming your <Tail /> telescope
                    at one of them.
                  </P>
                  <P>
                    <Tail /> is the telescope on your <Moon />. It is a
                    separate instrument, always running. It does not make
                    light; it lets you{" "}
                    <span className="font-medium">see</span>. Through it, other{" "}
                    <Sun /> appear. Through theirs, your <Sun /> appears. Same
                    private sky, both directions.
                  </P>
                  <P>
                    When your <Moon /> lines up with their <Sun />, that is an{" "}
                    <EclipseName />: you are seeing and controlling their desktop.
                  </P>
                </div>

                <div aria-hidden className="h-px bg-moonlight" />

                <div className="flex flex-col items-center gap-6">
                  <p className="text-sm font-medium tracking-widest text-moonlight/70">
                    Path of an Eclipse session
                  </p>
                  <div className="flex flex-wrap items-center justify-center gap-2">
                    <Chip icon="moon-solid" tone="moonlight">
                      your Moonlight
                    </Chip>
                    <span className="text-moonlight/70" aria-hidden>
                      →
                    </span>
                    <Chip icon="chart-network" tone="tailscale">
                      Tailscale
                    </Chip>
                    <span className="text-moonlight/70" aria-hidden>
                      →
                    </span>
                    <Chip icon="sun-solid" tone="sunshine">
                      their Sunshine
                    </Chip>
                  </div>
                </div>

                <EclipseFooterGif />
              </div>
            </div>
          </div>
        </DialogPanel>
      </div>
    </Dialog>
  );
}
