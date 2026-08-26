import { useRef } from "react";
import {
  Dialog,
  DialogBackdrop,
  DialogPanel,
  DialogTitle,
} from "@headlessui/react";
import { Button } from "./Button";

export const TLS_CERT_CHANGED_PREFIX = "ECLIPSE_TLS_CERT_CHANGED:";

export function parseTlsCertChangedHost(err: unknown): string | null {
  const text =
    typeof err === "string"
      ? err
      : err instanceof Error
        ? err.message
        : String(err);
  const idx = text.indexOf(TLS_CERT_CHANGED_PREFIX);
  if (idx < 0) {
    return null;
  }
  const ip = text
    .slice(idx + TLS_CERT_CHANGED_PREFIX.length)
    .trim()
    .split(/\s/)[0];
  return ip || null;
}

type Props = {
  ip: string;
  hostname: string | null;
  onTrust: () => void;
  onCancel: () => void;
};

export function CertTrustDialog({ ip, hostname, onTrust, onCancel }: Props) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  const who = hostname ? `${hostname} (${ip})` : ip;
  return (
    <Dialog
      open
      onClose={onCancel}
      initialFocus={cancelRef}
      className="relative z-50"
    >
      <DialogBackdrop className="fixed inset-0 bg-black/50" />
      <div className="fixed inset-0 flex items-center justify-center p-6">
        <DialogPanel className="w-full max-w-md border border-sunshine bg-eclipse p-5 text-moonlight outline-none">
          <DialogTitle className="text-sm font-medium uppercase tracking-widest text-sunshine">
            Host certificate
          </DialogTitle>
          <p className="mt-3 text-sm leading-relaxed">
            This host’s certificate changed. This is usually due to a new
            install/re-install of the Eclipse application on the Host&apos;s PC.
            Trust this Host?
          </p>
          <p className="mt-3 text-sm text-moonlight/70">{who}</p>
          <div className="mt-5 flex justify-end gap-2">
            <Button ref={cancelRef} variant="ghost" onClick={onCancel}>
              Cancel
            </Button>
            <Button variant="sunshine" onClick={onTrust}>
              Trust this Host
            </Button>
          </div>
        </DialogPanel>
      </div>
    </Dialog>
  );
}
