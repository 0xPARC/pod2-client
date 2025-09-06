import { cn } from "@/lib/utils";

export type RarityType = "NORM" | "RARE" | "EPIC" | "LGND" | "MYTH" | "GOD" | "????" | "ART" | "JUNK";

interface RarityBadgeProps {
  rarity: RarityType;
  size?: "sm" | "md" | "lg";
  variant?: "solid" | "outline";
  className?: string;
}

const RARITY_COLORS = {
  NORM: "bg-green-600",      // #138B41
  RARE: "bg-blue-600",       // #3D85C8
  EPIC: "bg-purple-600",     // #7614C0
  LGND: "bg-orange-500",     // #F90
  MYTH: "bg-yellow-500",     // #FFD700
  GOD: "bg-red-500",         // #FF6B6B
  "????": "bg-gray-500",     // #6C757D
  ART: "bg-pink-500",        // #E91E63
  JUNK: "bg-amber-800"       // #8B4513
} as const;

const SIZE_CLASSES = {
  sm: "px-2 py-1 text-xs",
  md: "px-3 py-1.5 text-sm",
  lg: "px-4 py-2 text-base"
} as const;

export function RarityBadge({ 
  rarity, 
  size = "md", 
  variant = "solid", 
  className 
}: RarityBadgeProps) {
  const baseClasses = "inline-flex items-center justify-center rounded font-semibold text-white";
  const colorClass = RARITY_COLORS[rarity];
  const sizeClass = SIZE_CLASSES[size];
  
  const variantClasses = variant === "outline" 
    ? "border-2 border-current bg-transparent text-current"
    : "";

  return (
    <span 
      className={cn(
        baseClasses,
        colorClass,
        sizeClass,
        variantClasses,
        className
      )}
    >
      {rarity}
    </span>
  );
}
