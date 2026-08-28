import type { HTMLAttributes } from "react";
import analytics from "../assets/icons/analytics.svg?raw";
import chartNetwork from "../assets/icons/chart-network.svg?raw";
import checkCircle from "../assets/icons/check-circle.svg?raw";
import clock from "../assets/icons/clock.svg?raw";
import cog from "../assets/icons/cog.svg?raw";
import command from "../assets/icons/command.svg?raw";
import expand from "../assets/icons/expand.svg?raw";
import infoCircle from "../assets/icons/info-circle.svg?raw";
import minus from "../assets/icons/minus.svg?raw";
import questionCircle from "../assets/icons/question-circle.svg?raw";
import moonSolid from "../assets/icons/moon-solid.svg?raw";
import sparklesSolid from "../assets/icons/sparkles-solid.svg?raw";
import spinner from "../assets/icons/spinner.svg?raw";
import refresh from "../assets/icons/refresh.svg?raw";
import sunSolid from "../assets/icons/sun-solid.svg?raw";
import times from "../assets/icons/times.svg?raw";
import timesCircle from "../assets/icons/times-circle.svg?raw";

const ICONS = {
  "sun-solid": sunSolid,
  "moon-solid": moonSolid,
  "sparkles-solid": sparklesSolid,
  "chart-network": chartNetwork,
  "check-circle": checkCircle,
  "times-circle": timesCircle,
  spinner,
  refresh,
  minus,
  expand,
  times,
  cog,
  command,
  "info-circle": infoCircle,
  "question-circle": questionCircle,
  analytics,
  clock,
} as const;

export type PixelIconName = keyof typeof ICONS;

type Props = HTMLAttributes<HTMLSpanElement> & {
  name: PixelIconName;
  size?: number;
};

function coloredSvg(svg: string): string {
  return svg
    .replace(/<\?xml[^?]*\?>/, "")
    .replace(/\s+id="[^"]*"/g, "")
    .replace(
      /<svg\b([^>]*)>/,
      '<svg$1 fill="currentColor" width="100%" height="100%" shape-rendering="crispEdges">',
    );
}

export function PixelIcon({
  name,
  size = 16,
  className = "",
  ...rest
}: Props) {
  return (
    <span
      aria-hidden
      className={`inline-block shrink-0 align-middle ${className}`}
      style={{ width: size, height: size, imageRendering: "pixelated" }}
      dangerouslySetInnerHTML={{ __html: coloredSvg(ICONS[name]) }}
      {...rest}
    />
  );
}
