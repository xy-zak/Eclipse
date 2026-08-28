import type { ReactNode } from "react";
import { DialogTitle } from "@headlessui/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { PageOverlay } from "./PageOverlay";
import { PixelIcon, type PixelIconName } from "./PixelIcon";

type Props = {
  open: boolean;
  onClose: () => void;
};

const TONE = {
  moonlight: "text-moonlight",
  sunshine: "text-sunshine",
  tailscale: "text-tailscale",
} as const;

const ECLIPSE_GITHUB = "https://github.com/xy-zak/Eclipse";
const ECLIPSE_LICENSE = `${ECLIPSE_GITHUB}/blob/main/LICENSE`;
const MOONLIGHT_GITHUB = "https://github.com/moonlight-stream/moonlight-qt";
const SUNSHINE_GITHUB = "https://github.com/LizardByte/Sunshine";
const TAILSCALE_SITE = "https://tailscale.com";
const CONTACT_MAIL = "mailto:itsonlyzak@hotmail.com";

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

function P({ children }: { children: ReactNode }) {
  return <p className="text-sm leading-relaxed text-moonlight">{children}</p>;
}

function Heading({ children }: { children: string }) {
  return (
    <h3 className="text-sm font-medium tracking-widest text-moonlight/70">
      {children}
    </h3>
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

const LIBRARIES: { name: string; license: string }[] = [
  { name: "Tauri", license: "MIT / Apache-2.0" },
  { name: "React", license: "MIT" },
  { name: "Qt", license: "LGPL-3.0" },
  { name: "FFmpeg", license: "LGPL / GPL" },
  { name: "SDL", license: "zlib" },
  { name: "Headless UI", license: "MIT" },
  { name: "Tailwind CSS", license: "MIT" },
  { name: "Pixel Icon Library", license: "MIT" },
];

const PLATFORMS: { name: string; package: string; soon?: boolean }[] = [
  { name: "Windows 11", package: ".exe" },
  { name: "Arch Linux", package: ".pkg.tar.zst" },
  { name: "Debian / Ubuntu", package: ".deb" },
  { name: "Fedora", package: ".rpm" },
  { name: "macOS", package: "coming soon", soon: true },
];

export function AboutOverlay({ open, onClose }: Props) {
  return (
    <PageOverlay open={open} onClose={onClose}>
      <header className="flex flex-col items-center gap-3 text-center">
        <DialogTitle className="font-title text-4xl font-normal tracking-tight sm:text-5xl">
          about
        </DialogTitle>
        <p className="max-w-xl text-sm leading-relaxed text-moonlight/70">
          free, open-source, and not locked to one operating system
        </p>
      </header>

      <section className="flex flex-col gap-3">
        <Heading>LICENSE</Heading>
        <P>
          <Named icon="sparkles-solid" tone="moonlight">
            Eclipse
          </Named>{" "}
          is free and open source, licensed under the GNU GPL-3.0 — the same
          copyleft as{" "}
          <Named icon="moon-solid" tone="moonlight">
            Moonlight
          </Named>{" "}
          and{" "}
          <Named icon="sun-solid" tone="sunshine">
            Sunshine
          </Named>
          . You may use, study, share, and change it, so long as those same
          freedoms travel with the source.{" "}
          <ExtLink href={ECLIPSE_LICENSE}>Eclipse LICENSE</ExtLink>
        </P>
      </section>

      <div aria-hidden className="h-px bg-moonlight" />

      <section className="flex flex-col gap-3">
        <Heading>SUPPORTED OPERATING SYSTEMS</Heading>
        <ul className="grid grid-cols-1 gap-x-8 gap-y-2 text-sm sm:grid-cols-2">
          {PLATFORMS.map((os) => (
            <li
              key={os.name}
              className="flex items-baseline justify-between gap-4 border-b border-moonlight/20 pb-1"
            >
              <span className="text-moonlight">{os.name}</span>
              <span
                className={`shrink-0 ${os.soon ? "text-sunshine" : "text-moonlight/70"}`}
              >
                {os.package}
              </span>
            </li>
          ))}
        </ul>
      </section>

      <div aria-hidden className="h-px bg-moonlight" />

      <section className="flex flex-col gap-3">
        <Heading>MOONLIGHT &amp; SUNSHINE</Heading>
        <P>
          <Named icon="moon-solid" tone="moonlight">
            Moonlight
          </Named>{" "}
          (Moonlight-Qt) is GPL-3.0. It is the GameStream client Eclipse vendors
          to view a remote desktop.{" "}
          <ExtLink href={MOONLIGHT_GITHUB}>Moonlight GITHUB</ExtLink>
        </P>
        <P>
          <Named icon="sun-solid" tone="sunshine">
            Sunshine
          </Named>{" "}
          is GPL-3.0. It is the self-hosted GameStream host Eclipse vendors so
          this desktop can be seen.{" "}
          <ExtLink href={SUNSHINE_GITHUB}>Sunshine GITHUB</ExtLink>
        </P>
      </section>

      <div aria-hidden className="h-px bg-moonlight" />

      <section className="flex flex-col gap-3">
        <Heading>OTHER LIBRARIES</Heading>
        <ul className="grid grid-cols-1 gap-x-8 gap-y-2 text-sm sm:grid-cols-2">
          {LIBRARIES.map((lib) => (
            <li
              key={lib.name}
              className="flex items-baseline justify-between gap-4 border-b border-moonlight/20 pb-1"
            >
              <span className="text-moonlight">{lib.name}</span>
              <span className="shrink-0 text-moonlight/70">{lib.license}</span>
            </li>
          ))}
        </ul>
      </section>

      <div aria-hidden className="h-px bg-moonlight" />

      <section className="flex flex-col gap-3">
        <Heading>TAILSCALE</Heading>
        <P>
          <Named icon="chart-network" tone="tailscale">
            Tailscale
          </Named>{" "}
          is third-party software with its own license. Eclipse does not
          package, ship, or relicense it. We only call the Tailscale CLI already
          on your machine. Install and license it from{" "}
          <ExtLink href={TAILSCALE_SITE}>tailscale.com</ExtLink>.
        </P>
      </section>

      <div aria-hidden className="h-px bg-moonlight" />

      <section className="flex flex-col gap-3">
        <Heading>WHY THIS EXISTS</Heading>
        <P>
          Small, useful, free software that is not locked to one operating
          system is a public good.{" "}
          <Named icon="sparkles-solid" tone="moonlight">
            Eclipse
          </Named>{" "}
          exists because the developers of{" "}
          <Named icon="sun-solid" tone="sunshine">
            Sunshine
          </Named>{" "}
          and{" "}
          <Named icon="moon-solid" tone="moonlight">
            Moonlight
          </Named>{" "}
          already did the hard work of capturing, encoding, and presenting a
          remote desktop — and because a mesh of other libraries, built in the
          open, made a control plane like this possible. WireGuard, which
          carries{" "}
          <Named icon="chart-network" tone="tailscale">
            Tailscale
          </Named>
          's tunnels, is part of that same tradition: a small tool that does one
          thing well, on every machine that needs it. Thank you to those
          developers, and to everyone who ships software you can read, run, and
          take with you.
        </P>
      </section>

      <div aria-hidden className="h-px bg-moonlight" />

      <section className="flex flex-col gap-3">
        <Heading>CONTACT</Heading>
        <P>Zak Silver-Lennard</P>
        <P>
          <ExtLink href={CONTACT_MAIL}>itsonlyzak@hotmail.com</ExtLink>
        </P>
      </section>

      <div aria-hidden className="h-px bg-moonlight" />

      <section className="flex flex-col gap-3">
        <Heading>SOURCE</Heading>
        <P>
          <ExtLink href={ECLIPSE_GITHUB}>Eclipse GITHUB</ExtLink>
        </P>
      </section>
    </PageOverlay>
  );
}
