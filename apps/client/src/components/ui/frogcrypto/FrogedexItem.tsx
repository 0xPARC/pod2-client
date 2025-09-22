import { RarityBadge, RarityType } from "./RarityBadge";
import { cn } from "@/lib/utils";

interface FrogedexItemProps {
  frogId: string;
  name: string;
  rarity: RarityType;
  seen: boolean;
  imageUrl?: string;
  className?: string;
}

export function FrogedexItem({ 
  frogId, 
  name, 
  rarity, 
  seen, 
  imageUrl, 
  className 
}: FrogedexItemProps) {
  return (
    <div
      className={cn(
        "flex items-center gap-4 p-4",
        className
      )}
    >
      {/* Frog Image */}
      <div className="w-16 h-16 flex-shrink-0">
        {seen && imageUrl ? (
          <img
            src={imageUrl}
            alt={name}
            className="w-full h-full object-contain"
          />
        ) : (
          <div className="w-full h-full bg-gray-200 rounded flex items-center justify-center">
            <span className="text-gray-400">?</span>
          </div>
        )}
      </div>

      {/* Frog Info */}
      <div className="flex-1">
        <div className="text-lg font-semibold">
          #{frogId} {seen ? name : "???"}
        </div>
        <RarityBadge
          rarity={rarity}
          size="sm"
          className={seen ? "" : "opacity-50"}
        />
      </div>
    </div>
  );
}
