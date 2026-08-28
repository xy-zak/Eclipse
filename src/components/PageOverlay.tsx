import { useRef, type ReactNode } from "react";
import {
  Dialog,
  DialogBackdrop,
  DialogPanel,
} from "@headlessui/react";
import { PixelIcon } from "./PixelIcon";
import { EclipseFooterGif } from "./EclipseFooterGif";

type Props = {
  open: boolean;
  onClose: () => void;
  children: ReactNode;
};

export function PageOverlay({ open, onClose, children }: Props) {
  const closeRef = useRef<HTMLButtonElement>(null);

  return (
    <Dialog
      open={open}
      onClose={onClose}
      initialFocus={closeRef}
      className="relative z-50"
    >
      <DialogBackdrop
        transition
        className="fixed inset-x-0 bottom-0 top-9 bg-black/50 transition duration-200 ease-out data-closed:opacity-0"
      />
      <div className="fixed inset-x-0 bottom-0 top-9 flex items-center justify-center">
        <DialogPanel className="flex h-[90%] w-[90%] flex-col border border-moonlight bg-eclipse outline-none">
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
                {children}
                <EclipseFooterGif />
              </div>
            </div>
          </div>
        </DialogPanel>
      </div>
    </Dialog>
  );
}
