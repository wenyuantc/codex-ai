import { cn } from "@/lib/utils";

interface KbdProps {
  children: React.ReactNode;
  className?: string;
  variant?: "default" | "outline" | "subtle" | "primary";
  size?: "sm" | "xs";
}

const variantStyles: Record<string, string> = {
  default: "bg-muted text-muted-foreground border-border dark:bg-white/10 dark:text-white",
  outline: "bg-background text-muted-foreground border-border dark:text-white",
  subtle: "bg-accent/40 text-muted-foreground border-transparent dark:bg-white/12 dark:text-white",
  primary: "bg-primary-foreground/15 text-primary-foreground border-transparent",
};

const sizeStyles: Record<string, string> = {
  sm: "h-5 min-w-5 px-1.5 text-[11px]",
  xs: "h-4 min-w-4 px-1 text-[10px]",
};

export function Kbd({ children, className, variant = "subtle", size = "sm" }: KbdProps) {
  return (
    <kbd
      className={cn(
        "inline-flex items-center justify-center rounded border font-mono font-medium leading-none",
        variantStyles[variant],
        sizeStyles[size],
        className,
      )}
    >
      {children}
    </kbd>
  );
}
