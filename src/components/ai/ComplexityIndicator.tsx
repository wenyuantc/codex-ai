import { cn } from "@/lib/utils";

interface ComplexityIndicatorProps {
  complexity: number | null;
  breakdown?: string;
  className?: string;
}

function getComplexityColor(score: number): string {
  if (score <= 3) return "bg-green-500";
  if (score <= 6) return "bg-yellow-500";
  if (score <= 8) return "bg-orange-500";
  return "bg-red-500";
}

function getComplexityLabel(score: number): string {
  if (score <= 3) return "简单";
  if (score <= 6) return "中等";
  if (score <= 8) return "复杂";
  return "极高";
}

export function ComplexityIndicator({
  complexity,
  breakdown,
  className,
}: ComplexityIndicatorProps) {
  if (complexity === null || complexity === undefined) {
    return null;
  }

  const clampedScore = Math.max(1, Math.min(10, complexity));
  const color = getComplexityColor(clampedScore);
  const label = getComplexityLabel(clampedScore);

  return (
    <div
      className={cn("flex items-center gap-1.5", className)}
      title={breakdown ?? `${clampedScore}/10 ${label}`}
    >
      <div className="flex gap-px">
        {Array.from({ length: 10 }).map((_, i) => (
          <div
            key={i}
            className={cn("w-1.5 h-3 rounded-sm", i < clampedScore ? color : "bg-muted")}
          />
        ))}
      </div>
      <span className="text-xs text-muted-foreground">
        {clampedScore}/10 {label}
      </span>
    </div>
  );
}
