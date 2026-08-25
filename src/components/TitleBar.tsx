import { getCurrentWindow } from "@tauri-apps/api/window";
import { PixelIcon } from "./PixelIcon";

export function TitleBar() {
  const win = () => getCurrentWindow();

  return (
    <header className="flex h-9 shrink-0 items-stretch border-b border-moonlight bg-eclipse">
      <div className="flex-1" data-tauri-drag-region />
      <div className="flex">
        <button
          type="button"
          aria-label="Minimise"
          className="flex w-10 items-center justify-center border-l border-moonlight text-moonlight hover:bg-moonlight hover:text-eclipse"
          onClick={() => void win().minimize().catch(() => undefined)}
        >
          <PixelIcon name="minus" size={14} />
        </button>
        <button
          type="button"
          aria-label="Fullscreen"
          className="flex w-10 items-center justify-center border-l border-moonlight text-moonlight hover:bg-moonlight hover:text-eclipse"
          onClick={() => {
            void (async () => {
              const w = win();
              await w.setFullscreen(!(await w.isFullscreen()));
            })().catch(() => undefined);
          }}
        >
          <PixelIcon name="expand" size={14} />
        </button>
        <button
          type="button"
          aria-label="Hide to tray"
          className="flex w-10 items-center justify-center border-l border-moonlight text-moonlight hover:bg-fail hover:text-eclipse"
          onClick={() => void win().close().catch(() => undefined)}
        >
          <PixelIcon name="times" size={14} />
        </button>
      </div>
    </header>
  );
}
