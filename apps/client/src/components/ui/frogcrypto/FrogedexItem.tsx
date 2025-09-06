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

      
      <RarityBadge 
        rarity={rarity} 
        size="sm"
        className={seen ? "" : "opacity-50"}
      />
      

      
    </div>
  );
}
