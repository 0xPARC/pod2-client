import { RarityType } from "./RarityBadge";
import { cn } from "@/lib/utils";

interface FrogCardHeaderProps {
  rarity: RarityType;
  frogNumber: string;
  frogName?: string;
  className?: string;
}

const RARITY_COLORS = {
  NORM: "bg-green-600", // #138B41
  RARE: "bg-blue-600", // #3D85C8
  EPIC: "bg-purple-600", // #7614C0
  LGND: "bg-orange-500", // #F90
  MYTH: "bg-yellow-500", // #FFD700
  GOD: "bg-red-500", // #FF6B6B
  "????": "bg-gray-500", // #6C757D
  ART: "bg-pink-500", // #E91E63
  JUNK: "bg-amber-800" // #8B4513
} as const;

export function FrogCardHeader({
  rarity,
  frogNumber,
  frogName,
  className
}: FrogCardHeaderProps) {
  const colorClass = RARITY_COLORS[rarity];

  return (
    <div className={cn("relative", className)}>
      {/* Main header */}
      <div
        className={cn(
          "h-12 rounded-t-xl flex items-center justify-center",
          colorClass
        )}
      >
        <div
          className="text-white font-semibold text-base tracking-wide"
          style={{
            fontFamily: "var(--font-sans)",
            fontSize: "16px",
            fontWeight: 600,
            lineHeight: "20px",
            letterSpacing: "1px"
          }}
        >
          #{frogNumber} {frogName ? frogName.toUpperCase() : `${rarity} FROG`}
        </div>
      </div>
    </div>
  );
}
