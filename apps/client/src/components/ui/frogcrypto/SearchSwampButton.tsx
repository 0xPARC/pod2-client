import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface SearchSwampButtonProps {
  onClick: () => void;
  disabled?: boolean;
  loading?: boolean;
  waitText?: string;
  className?: string;
}

export function SearchSwampButton({
  onClick,
  disabled = false,
  loading = false,
  waitText = "",
  className
}: SearchSwampButtonProps) {
  return (
    <Button
      className={cn(
        "hover:bg-gray-500 text-white font-medium text-base tracking-wide transition-all duration-200",
        disabled && "opacity-50 cursor-not-allowed",
        loading && "opacity-75 cursor-wait",
        className
      )}
      onClick={onClick}
      disabled={disabled || loading}
      style={{
        fontFamily: "var(--font-sans)",
        fontSize: "16px",
        fontWeight: 500,
        letterSpacing: "0.32px",
        width: "350px",
        height: "28px",
        marginTop: "25px",
        backgroundColor: "#325F58"
      }}
    >
      {loading ? "SEARCHING..." : `SEARCH SWAMP${waitText}`}
    </Button>
  );
}
