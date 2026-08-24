import type { ReactNode } from "react";

type Accent = "moonlight" | "sunshine";

type Props = {
  accent: Accent;
  title: string;
  icon?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
};

export function Panel({ accent, title, icon, actions, children }: Props) {
  const border = accent === "sunshine" ? "border-sunshine" : "border-moonlight";
  const titleColor = accent === "sunshine" ? "text-sunshine" : "text-moonlight";
  return (
    <section className={`border bg-eclipse p-5 ${border}`}>
      <div className="mb-4 flex items-center justify-between gap-3">
        <h2
          className={`flex items-center gap-2 text-sm font-medium uppercase tracking-widest ${titleColor}`}
        >
          {icon}
          {title}
        </h2>
        {actions}
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
    <div className="flex items-baseline justify-between gap-4 py-1.5 text-sm">
      <span className="flex items-center gap-2 text-moonlight/70">
        {icon ? <span className={iconColor}>{icon}</span> : null}
        {label}
      </span>
      <span className="text-right font-medium">{value}</span>
    </div>
  );
}
