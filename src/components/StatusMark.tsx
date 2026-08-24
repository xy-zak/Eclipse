import type { ReactNode } from "react";

type Props = {
  ok: boolean;
  label: string;
  icon?: ReactNode;
  tone?: "moonlight" | "sunshine";
};

export function StatusMark({ ok, label, icon, tone = "moonlight" }: Props) {
  const iconColor = tone === "sunshine" ? "text-sunshine" : "text-moonlight";
  return (
    <span className="inline-flex items-center gap-2 text-sm">
      {icon ? <span className={iconColor}>{icon}</span> : null}
      <span className="text-moonlight">{label}</span>
      <span className={ok ? "text-success" : "text-fail"}>
        {ok ? "active" : "inactive"}
      </span>
    </span>
  );
}
