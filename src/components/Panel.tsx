import type { ReactNode } from "react";

type Accent = "moonlight" | "sunshine";

type Props = {
  accent: Accent;
  title: string;
  icon?: ReactNode;
  actions?: ReactNode;
  className?: string;
  children: ReactNode;
};

export function Panel({
  accent,
  title,
  icon,
  actions,
  className = "",
  children,
}: Props) {
  const border = accent === "sunshine" ? "border-sunshine" : "border-moonlight";
  const titleColor = accent === "sunshine" ? "text-sunshine" : "text-moonlight";
  return (
    <section className={`relative min-w-0 border bg-eclipse p-5 ${border} ${className}`}>
      <div className="mb-4 flex min-w-0 items-center justify-between gap-3">
        <h2
          className={`flex min-w-0 items-center gap-2 text-sm font-medium uppercase tracking-widest ${titleColor}`}
        >
          {icon}
          <span className="truncate">{title}</span>
        </h2>
        {actions ? <div className="shrink-0">{actions}</div> : null}
      </div>
      {children}
    </section>
  );
}

type RowProps = {
  label: string;
  value: string;
  icon?: ReactNode;
  accent?: Accent;
};

export function InfoRow({ label, value, icon, accent = "moonlight" }: RowProps) {
  const iconColor = accent === "sunshine" ? "text-sunshine" : "text-moonlight";
  return (
    <div className="flex min-w-0 items-baseline justify-between gap-4 py-1.5 text-sm">
      <span className="flex shrink-0 items-center gap-2 text-moonlight/70">
        {icon ? <span className={iconColor}>{icon}</span> : null}
        {label}
      </span>
      <span className="min-w-0 text-right font-medium wrap-break-word">
        {value}
      </span>
    </div>
  );
}
