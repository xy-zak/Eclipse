import type { DisplayInfo } from "./types";

export function dash(value: string | number | null | undefined): string {
  if (value === null || value === undefined || value === "") {
    return "—";
  }
  return String(value);
}

export function formatDisplays(displays: DisplayInfo[] | undefined | null): string {
  if (!displays?.length) {
    return "—";
  }
  return displays
    .map((d) =>
      d.name ? `${d.name} ${d.width}×${d.height}` : `${d.width}×${d.height}`,
    )
    .join(", ");
}
