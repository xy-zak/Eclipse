import { forwardRef, type ButtonHTMLAttributes } from "react";

type Variant = "sunshine" | "moonlight" | "fail" | "ghost";

const VARIANT: Record<Variant, string> = {
  sunshine:
    "border-sunshine bg-sunshine text-eclipse enabled:hover:border-success enabled:hover:bg-success disabled:border-moonlight disabled:bg-eclipse disabled:text-moonlight disabled:opacity-40",
  moonlight:
    "border-moonlight bg-moonlight text-eclipse hover:bg-[#c4c4f0] disabled:bg-eclipse disabled:text-moonlight disabled:opacity-40",
  fail: "border-fail bg-fail text-eclipse hover:bg-[#d44545] disabled:opacity-40",
  ghost:
    "border-moonlight bg-eclipse text-moonlight hover:bg-moonlight hover:text-eclipse disabled:opacity-40",
};

type Props = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: Variant;
};

export const Button = forwardRef<HTMLButtonElement, Props>(function Button(
  { variant = "moonlight", className = "", type = "button", ...props },
  ref,
) {
  return (
    <button
      ref={ref}
      type={type}
      className={`inline-flex items-center justify-center gap-2 border px-3 py-1.5 text-sm font-medium disabled:cursor-not-allowed ${VARIANT[variant]} ${className}`}
      {...props}
    />
  );
});
