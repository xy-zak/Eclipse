import { useRef, useState } from "react";
import {
  Popover,
  PopoverBackdrop,
  PopoverButton,
  PopoverPanel,
} from "@headlessui/react";
import type { HostLockStatus } from "../types";
import { Button } from "./Button";
import { DROPDOWN_TRIGGER_OPEN, DROPDOWN_WIDTH } from "./dropdown";
import { PixelIcon } from "./PixelIcon";
import { StatusMark } from "./StatusMark";

const HOST_ADMIN_URL = "https://127.0.0.1:47990";

type Props = {
  sunshinePresent: boolean;
  sunshineRunning: boolean;
  keepHostOnExit: boolean;
  hostStartOnBoot: boolean;
  hostServiceInstalled: boolean;
  onOpenHostAdmin: () => void;
  onCopyWebUiPassword: () => Promise<void>;
  onSetHostRunning: (on: boolean) => Promise<void>;
  onSetKeepHostOnExit: (on: boolean) => Promise<void>;
  onSetHostStartOnBoot: (on: boolean) => Promise<void>;
  hostLock: HostLockStatus | null;
  onSetHostLockOnEnd: (on: boolean) => Promise<void>;
  onTestHostLock: () => Promise<void>;
};

export function SunshinePopover({
  sunshinePresent,
  sunshineRunning,
  keepHostOnExit,
  hostStartOnBoot,
  hostServiceInstalled,
  onOpenHostAdmin,
  onCopyWebUiPassword,
  onSetHostRunning,
  onSetKeepHostOnExit,
  onSetHostStartOnBoot,
  hostLock,
  onSetHostLockOnEnd,
  onTestHostLock,
}: Props) {
  const [copied, setCopied] = useState(false);
  const [hostBusy, setHostBusy] = useState(false);
  const [keepBusy, setKeepBusy] = useState(false);
  const [bootBusy, setBootBusy] = useState(false);
  const [lockBusy, setLockBusy] = useState(false);
  const copyTimer = useRef<number | null>(null);

  const copyPassword = () => {
    void onCopyWebUiPassword()
      .then(() => {
        setCopied(true);
        if (copyTimer.current != null) {
          window.clearTimeout(copyTimer.current);
        }
        copyTimer.current = window.setTimeout(() => setCopied(false), 2000);
      })
      .catch(() => {
        setCopied(false);
      });
  };

  const toggleHosting = (on: boolean) => {
    setHostBusy(true);
    void onSetHostRunning(on).finally(() => setHostBusy(false));
  };

  const setKeepOnExit = (on: boolean) => {
    setKeepBusy(true);
    void onSetKeepHostOnExit(on).finally(() => setKeepBusy(false));
  };

  const setStartOnBoot = (on: boolean) => {
    setBootBusy(true);
    void onSetHostStartOnBoot(on).finally(() => setBootBusy(false));
  };

  const setLockOnEnd = (on: boolean) => {
    setLockBusy(true);
    void onSetHostLockOnEnd(on).finally(() => setLockBusy(false));
  };

  const testLock = () => {
    setLockBusy(true);
    void onTestHostLock().finally(() => setLockBusy(false));
  };

  const hostToggleDisabled = hostBusy || !sunshinePresent;
  const bootToggleDisabled = bootBusy || !hostServiceInstalled;

  return (
    <Popover className={({ open }) => (open ? "relative z-50 isolate" : "")}>
      {({ open }) => (
        <>
          <PopoverButton
            aria-label="Host Sunshine"
            className={`inline-flex h-9 w-9 items-center justify-center border border-sunshine bg-eclipse text-sunshine hover:bg-sunshine hover:text-eclipse ${
              open ? DROPDOWN_TRIGGER_OPEN : ""
            }`}
          >
            <PixelIcon name="sun-solid" size={16} />
          </PopoverButton>
          <PopoverBackdrop
            transition
            className="fixed inset-0 z-40 bg-black/50 transition duration-200 ease-out data-closed:opacity-0"
          />
          <PopoverPanel
            modal={false}
            anchor="bottom end"
            className={`z-50 mt-1 max-h-[min(36rem,calc(100dvh-5rem))] overflow-y-auto ${DROPDOWN_WIDTH} border border-sunshine bg-eclipse p-4 text-left text-sm text-sunshine outline-none`}
          >
            <p className="font-medium uppercase tracking-widest text-moonlight/70">
              This PC’s Sunshine
            </p>
            <p className="mt-2 text-moonlight/70">
              Sunshine is the host engine on this machine. While Host streaming
              is on, other machines on your Tailscale network can control this
              desktop. Turn it off here or from the tray when you do not want
              that.
            </p>
            <div className="mt-3 flex flex-col gap-2">
              <StatusMark
                ok={sunshinePresent}
                label={
                  sunshinePresent
                    ? "Sunshine binary is present"
                    : "Sunshine binary is missing"
                }
                tone="sunshine"
              />
              <StatusMark
                ok={sunshineRunning}
                label={
                  sunshineRunning
                    ? "Sunshine is active"
                    : "Sunshine is inactive"
                }
                tone="sunshine"
              />
            </div>
            {sunshineRunning ? (
              <Button
                variant="fail"
                className="mt-3 w-full"
                disabled={hostToggleDisabled}
                onClick={() => toggleHosting(false)}
              >
                {hostBusy ? "Stopping…" : "Turn off hosting"}
              </Button>
            ) : (
              <button
                type="button"
                disabled={hostToggleDisabled}
                onClick={() => toggleHosting(true)}
                className="mt-3 w-full border border-sunshine bg-eclipse px-3 py-1.5 text-sm font-medium text-sunshine hover:border-success hover:bg-success hover:text-eclipse disabled:cursor-not-allowed disabled:opacity-40"
              >
                {hostBusy ? "Starting…" : "Turn on hosting"}
              </button>
            )}
            <p className="mt-3 text-moonlight/70">
              Closing the window hides Eclipse to the tray. Quit Eclipse exits
              the app. Leave the host running after quit so other PCs can still
              Connect, or stop it when you quit.
            </p>
            {keepHostOnExit ? (
              <Button
                variant="fail"
                className="mt-3 w-full"
                disabled={keepBusy}
                onClick={() => setKeepOnExit(false)}
              >
                Stop Sunshine Host when Eclipse closes
              </Button>
            ) : (
              <button
                type="button"
                disabled={keepBusy}
                onClick={() => setKeepOnExit(true)}
                className="mt-3 w-full border border-sunshine bg-eclipse px-3 py-1.5 text-sm font-medium text-sunshine hover:border-success hover:bg-success hover:text-eclipse disabled:cursor-not-allowed disabled:opacity-40"
              >
                Keep Sunshine Host running in background
              </button>
            )}
            <p className="mt-3 text-moonlight/70">
              Auto-start hosting on reboot starts Sunshine when this PC boots,
              even before anyone logs in. Turn it off if you only want hosting
              when you start it from Eclipse.
            </p>
            {hostStartOnBoot ? (
              <Button
                variant="fail"
                className="mt-3 w-full"
                disabled={bootToggleDisabled}
                onClick={() => setStartOnBoot(false)}
              >
                {bootBusy
                  ? "Updating…"
                  : "Turn off auto-start hosting on reboot"}
              </Button>
            ) : (
              <button
                type="button"
                disabled={bootToggleDisabled}
                onClick={() => setStartOnBoot(true)}
                className="mt-3 w-full border border-sunshine bg-eclipse px-3 py-1.5 text-sm font-medium text-sunshine hover:border-success hover:bg-success hover:text-eclipse disabled:cursor-not-allowed disabled:opacity-40"
              >
                {bootBusy ? "Updating…" : "Auto-start hosting on reboot"}
              </button>
            )}
            {hostLock?.supported ? (
              <>
                <p className="mt-3 text-moonlight/70">
                  Lock this PC when a stream ends, so nobody at this desk sees
                  your session. It waits a few seconds so switching displays
                  does not lock, and also covers dropped connections. Needs
                  Eclipse running in the tray.
                </p>
                {hostLock.on ? (
                  <Button
                    variant="fail"
                    className="mt-3 w-full"
                    disabled={lockBusy}
                    onClick={() => setLockOnEnd(false)}
                  >
                    Stop locking this PC when a stream ends
                  </Button>
                ) : (
                  <button
                    type="button"
                    disabled={lockBusy}
                    onClick={() => setLockOnEnd(true)}
                    className="mt-3 w-full border border-sunshine bg-eclipse px-3 py-1.5 text-sm font-medium text-sunshine hover:border-success hover:bg-success hover:text-eclipse disabled:cursor-not-allowed disabled:opacity-40"
                  >
                    Lock this PC when a stream ends
                  </button>
                )}
                <button
                  type="button"
                  disabled={lockBusy}
                  onClick={testLock}
                  className="mt-2 w-full border border-sunshine bg-eclipse px-3 py-1.5 text-sm font-medium text-sunshine hover:bg-sunshine hover:text-eclipse disabled:cursor-not-allowed disabled:opacity-40"
                >
                  {lockBusy ? "Locking…" : "Test lock now"}
                </button>
                {hostLock.lastError ? (
                  <p className="mt-2 text-fail">{hostLock.lastError}</p>
                ) : null}
              </>
            ) : null}
            <p className="mt-3 text-moonlight/70">
              Host admin is the local Sunshine page at {HOST_ADMIN_URL}. The
              certificate is self-signed; the browser warning is expected.
              Connect does not use these credentials.
            </p>
            <button
              type="button"
              disabled={!sunshineRunning}
              onClick={onOpenHostAdmin}
              className="mt-3 w-full border border-sunshine bg-eclipse px-3 py-1.5 text-sm font-medium text-sunshine hover:bg-sunshine hover:text-eclipse disabled:cursor-not-allowed disabled:opacity-40"
            >
              Open host admin control panel
            </button>
            <dl className="mt-4 grid grid-cols-[auto_1fr] items-center gap-x-8 gap-y-2">
              <dt className="text-moonlight/70">Username</dt>
              <dd className="text-right font-medium">eclipse</dd>
              <dt className="text-moonlight/70">Password</dt>
              <dd className="text-right">
                <button
                  type="button"
                  onClick={copyPassword}
                  className="border border-sunshine bg-eclipse px-3 py-1 text-sm font-medium text-sunshine hover:bg-sunshine hover:text-eclipse"
                >
                  {copied ? "Copied" : "Copy host admin password"}
                </button>
              </dd>
            </dl>
          </PopoverPanel>
        </>
      )}
    </Popover>
  );
}
