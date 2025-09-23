import { cn } from "@/lib/utils";

interface CustomToggleProps {
  enabled: boolean;
  onToggle: (enabled: boolean) => void;
  className?: string;
}

export function CustomToggle({
  enabled,
  onToggle,
  className
}: CustomToggleProps) {
  return (
    <button
      className={cn(
        "relative inline-flex items-center rounded-full transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500",
        enabled ? "bg-green-600" : "bg-gray-500",
        className
      )}
      onClick={() => onToggle(!enabled)}
      style={{
        width: "66px",
        height: "32px",
        flexShrink: 0,
        borderRadius: "16px"
      }}
    >
      <span
        className={cn(
          "inline-block rounded-full bg-white shadow-lg transform transition-transform duration-200",
          enabled ? "translate-x-8" : "translate-x-1"
        )}
        style={{
          width: "24px",
          height: "24px"
        }}
      />
    </button>
  );
}
