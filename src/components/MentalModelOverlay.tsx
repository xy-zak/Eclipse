import { Fragment, useEffect, useState, type ReactNode } from "react";
import { DialogTitle } from "@headlessui/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { PixelIcon, type PixelIconName } from "./PixelIcon";
import { PageOverlay } from "./PageOverlay";

type Props = {
  open: boolean;
  onClose: () => void;
};

type GuideMode = "conceptual" | "technical";

const ECLIPSE_README =
  "https://github.com/xy-zak/Eclipse/blob/main/README.md";

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

function EclipseName() {
  return (
    <Named icon="sparkles-solid" tone="moonlight">
      Eclipse
    </Named>
  );
}

function ExtLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a
      href={href}
      className="text-moonlight/70 underline decoration-moonlight/40 underline-offset-4 hover:text-sunshine"
      onClick={(e) => {
        e.preventDefault();
        void openUrl(href).catch(() => {
          window.open(href, "_blank", "noopener");
        });
      }}
    >
      {children}
    </a>
  );
}

function ReadmeLink() {
  return (
    <p className="text-center text-sm text-moonlight/70">
      <ExtLink href={ECLIPSE_README}>Eclipse README</ExtLink>
    </p>
  );
}

function Code({ children }: { children: ReactNode }) {
  return <span className="text-moonlight/70">{children}</span>;
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

type Hop = {
  icon: PixelIconName;
  tone: keyof typeof TONE;
  label: string;
};

function PathLine({ hops }: { hops: Hop[] }) {
  return (
    <div className="flex flex-wrap items-center justify-center gap-2">
      {hops.map((hop, i) => (
        <Fragment key={`${hop.tone}-${hop.label}`}>
          {i > 0 ? (
            <span className="text-moonlight/70" aria-hidden>
              →
            </span>
          ) : null}
          <Chip icon={hop.icon} tone={hop.tone}>
            {hop.label}
          </Chip>
        </Fragment>
      ))}
    </div>
  );
}

type PathStep = Hop & { detail: string };

function VerticalPath({ hops }: { hops: PathStep[] }) {
  return (
    <ol className="flex w-full max-w-lg flex-col items-center">
      {hops.map((hop, i) => (
        <li
          key={`${hop.tone}-${hop.label}`}
          className="flex w-full flex-col items-center"
        >
          <Chip icon={hop.icon} tone={hop.tone}>
            {hop.label}
          </Chip>
          <p className="mt-1 text-center text-xs leading-relaxed text-moonlight/70">
            {hop.detail}
          </p>
          {i < hops.length - 1 ? (
            <span className="py-2 text-moonlight/70" aria-hidden>
              ↓
            </span>
          ) : null}
        </li>
      ))}
    </ol>
  );
}

function ModeToggle({
  mode,
  onChange,
}: {
  mode: GuideMode;
  onChange: (mode: GuideMode) => void;
}) {
  return (
    <div
      className="flex w-full max-w-md"
      role="group"
      aria-label="Explanation style"
    >
      {(
        [
          { value: "conceptual", label: "Conceptual" },
          { value: "technical", label: "Technical" },
        ] as const
      ).map((option, i) => {
        const selected = option.value === mode;
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={selected}
            onClick={() => onChange(option.value)}
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
  );
}

function ConceptualBody() {
  return (
    <>
      <ReadmeLink />
      <div className="flex flex-col gap-5">
        <P>
          <EclipseName /> is an app you install on each of your computers
          (along with the 3rd-party app{" "}
          <Named icon="chart-network" tone="tailscale">
            Tailscale
          </Named>
          ). Every PC can share its desktop with{" "}
          <Named icon="sun-solid" tone="sunshine">
            Sunshine
          </Named>
          , view another desktop with{" "}
          <Named icon="moon-solid" tone="moonlight">
            Moonlight
          </Named>
          , and reach the others over{" "}
          <Named icon="chart-network" tone="tailscale">
            Tailscale
          </Named>
          .
        </P>
        <P>
          This computer is the <UiLabel>CLIENT</UiLabel>: you use its{" "}
          <Named icon="moon-solid" tone="moonlight">
            Moonlight
          </Named>
          . <UiLabel>HOSTS</UiLabel> are other computers running{" "}
          <Named icon="sun-solid" tone="sunshine">
            Sunshine
          </Named>
          . Connect starts a{" "}
          <Named icon="moon-solid" tone="moonlight">
            Moonlight
          </Named>{" "}
          session so you see and control that desktop’s{" "}
          <Named icon="sun-solid" tone="sunshine">
            Sunshine
          </Named>
          . You can leave{" "}
          <Named icon="sun-solid" tone="sunshine">
            Sunshine
          </Named>{" "}
          running after you close <EclipseName />, so others can still connect
          to you.
        </P>
      </div>

      <div aria-hidden className="h-px bg-moonlight" />

      <div className="flex flex-col items-center gap-6">
        <p className="text-sm font-medium tracking-widest text-moonlight/70">
          Path of an Eclipse session
        </p>
        <PathLine
          hops={[
            { icon: "moon-solid", tone: "moonlight", label: "your Moonlight" },
            { icon: "chart-network", tone: "tailscale", label: "Tailscale" },
            { icon: "sun-solid", tone: "sunshine", label: "their Sunshine" },
          ]}
        />
      </div>
    </>
  );
}

function TechnicalBody() {
  return (
    <>
      <ReadmeLink />
      <div className="flex flex-col gap-5">
        <P>
          <EclipseName /> is a <Code>Tauri</Code> + <Code>React</Code> control
          plane. It does not capture, encode, or decode video. Those jobs belong
          to two bundled sidecars:{" "}
          <Named icon="sun-solid" tone="sunshine">
            Sunshine
          </Named>{" "}
          (host) and{" "}
          <Named icon="moon-solid" tone="moonlight">
            Moonlight-Qt
          </Named>{" "}
          (client: <Code>moonlight-common-c</Code>, <Code>FFmpeg</Code>,{" "}
          <Code>SDL</Code>).{" "}
          <Named icon="chart-network" tone="tailscale">
            Tailscale
          </Named>{" "}
          is not bundled — <Code>tailscaled</Code> is an OS{" "}
          <Code>WireGuard</Code> mesh <EclipseName /> talks to only through the{" "}
          <Code>CLI</Code>.
        </P>
        <P>
          The dashboard lists peers from{" "}
          <Code>tailscale status --json</Code> and marks a{" "}
          <UiLabel>HOSTS</UiLabel> row online when <Code>TCP :47990</Code>{" "}
          answers. Connect pairs over <Code>HTTPS :47990</Code> (
          <Code>POST /api/pin</Code>, <Code>TOFU</Code> cert pin) then runs{" "}
          <Code>moonlight stream</Code> at that peer’s <Code>CGNAT</Code> IPv4.
          This computer’s{" "}
          <Named icon="moon-solid" tone="moonlight">
            Moonlight
          </Named>{" "}
          is the <UiLabel>CLIENT</UiLabel>; you never remote into someone else’s
          viewer process.
        </P>
        <P>
          This PC also runs{" "}
          <Named icon="sun-solid" tone="sunshine">
            Sunshine
          </Named>
          : packaged installs use <Code>eclipse-host.service</Code> /{" "}
          <Code>SunshineService</Code> at boot; source builds spawn a child.
          Start and stop it from the <UiLabel>CLIENT</UiLabel> sun menu or the
          tray. Keep-on-exit leaves the host up after Quit <EclipseName /> so
          others can still Connect.
        </P>
      </div>

      <div aria-hidden className="h-px bg-moonlight" />

      <div className="flex flex-col items-center gap-6">
        <p className="text-sm font-medium tracking-widest text-moonlight/70">
          Network path of an <EclipseName /> Session
        </p>
        <div className="flex w-full flex-col items-center gap-8 sm:flex-row sm:items-start sm:justify-center sm:gap-12">
          <div className="flex w-full flex-col items-center gap-4 sm:w-auto">
            <p className="flex flex-wrap items-center justify-center gap-2 text-xs font-medium tracking-widest text-moonlight/70">
              <Named icon="moon-solid" tone="moonlight">
                Moonlight
              </Named>
              <span aria-hidden>→</span>
              <Named icon="sun-solid" tone="sunshine">
                Sunshine
              </Named>
            </p>
            <VerticalPath
              hops={[
                {
                  icon: "moon-solid",
                  tone: "moonlight",
                  label: "Moonlight-Qt",
                  detail: "GameStream client · FFmpeg · SDL · Qt",
                },
                {
                  icon: "sparkles-solid",
                  tone: "moonlight",
                  label: "HTTPS :47990 · RTSP :48010",
                  detail: "pair /api/pin, then stream setup",
                },
                {
                  icon: "chart-network",
                  tone: "tailscale",
                  label: "Tailscale TUN 100.x",
                  detail: "CGNAT 100.64.0.0/10 · tailscaled",
                },
                {
                  icon: "chart-network",
                  tone: "tailscale",
                  label: "WireGuard",
                  detail: "direct UDP after STUN, else DERP",
                },
                {
                  icon: "sun-solid",
                  tone: "sunshine",
                  label: "Sunshine",
                  detail: "host sidecar on the remote node",
                },
              ]}
            />
          </div>
          <div className="flex w-full flex-col items-center gap-4 sm:w-auto">
            <p className="flex flex-wrap items-center justify-center gap-2 text-xs font-medium tracking-widest text-moonlight/70">
              <Named icon="sun-solid" tone="sunshine">
                Sunshine
              </Named>
              <span aria-hidden>→</span>
              <Named icon="moon-solid" tone="moonlight">
                Moonlight
              </Named>
            </p>
            <VerticalPath
              hops={[
                {
                  icon: "sun-solid",
                  tone: "sunshine",
                  label: "Capture + encode",
                  detail: "DXGI / PipeWire / KMS · NVENC / AMF / QSV / VA-API",
                },
                {
                  icon: "sparkles-solid",
                  tone: "moonlight",
                  label: "UDP 47998–48010",
                  detail: "GameStream video, audio, and input",
                },
                {
                  icon: "chart-network",
                  tone: "tailscale",
                  label: "WireGuard",
                  detail: "same tunnel · STUN or DERP",
                },
                {
                  icon: "moon-solid",
                  tone: "moonlight",
                  label: "Moonlight-Qt",
                  detail: "D3D11VA / VA-API decode · Qt / SDL present",
                },
              ]}
            />
          </div>
        </div>
        <p className="max-w-3xl text-center text-sm leading-relaxed text-moonlight/70">
          Stream encryption is mandatory. Inbound <Code>GameStream</Code> is
          firewalled to <Code>100.64.0.0/10</Code>.
        </p>
      </div>
    </>
  );
}

export function MentalModelOverlay({ open, onClose }: Props) {
  const [mode, setMode] = useState<GuideMode>("conceptual");

  useEffect(() => {
    if (!open) {
      setMode("conceptual");
    }
  }, [open]);

  return (
    <PageOverlay open={open} onClose={onClose}>
      <header className="flex flex-col items-center gap-6 text-center">
        <DialogTitle className="font-title text-4xl font-normal tracking-tight sm:text-5xl">
          how this works
        </DialogTitle>
        <ModeToggle mode={mode} onChange={setMode} />
      </header>

      {mode === "conceptual" ? <ConceptualBody /> : <TechnicalBody />}
    </PageOverlay>
  );
}
