import { Button } from "@/components/ui/button";
import { FrogCardHeader } from "./FrogCardHeader";
import { FrogStats } from "./FrogStats";
import { RarityType } from "./RarityBadge";
import { cn } from "@/lib/utils";
import { useState } from "react";
import { Frog } from "@/lib/rpc";

interface FrogCardProps {
  frog: Frog;
  rarity: RarityType;
  index: number;
  onClick?: () => void;
  className?: string;
}

const temperaments = new Map<number, string>([
  [1, "N/A"],
  [2, "ANGY"],
  [3, "BORD"],
  [4, "CALM"],
  [7, "DARK"],
  [10, "HNGY"],
  [16, "SADG"],
  [18, "SLPY"]
]);

function vibeEntry(index: number | undefined): string {
  if (index == null) {
    return "???";
  }
  return temperaments.get(index as number) ?? "???";
}

export function FrogCard({ frog, rarity, index, onClick, className }: FrogCardProps) {
  const [expanded, setExpanded] = useState(false);
  const haveDesc = frog.derived !== null;

  return (
    <div 
      className={cn(
        "relative bg-white rounded-xl border-2 border-white shadow-lg cursor-pointer transition-all duration-300 hover:shadow-xl",
        className
      )}
      onClick={onClick}
      style={{
        width: '280px',
        height: '400px'
      }}
    >
      {/* Header */}
      <FrogCardHeader
        rarity={rarity}
        frogNumber={String(index + 1).padStart(2, '0')}
        frogName={frog.derived?.name}
      />

      {/* Frog Image */}
      <div className="absolute top-20 left-4 right-4 flex items-center justify-center">
        {haveDesc && frog.derived?.image_url && (
          <img
            src={frog.derived.image_url}
            className="max-w-full max-h-full object-contain"
            alt="Frog"
          />
        )}
      </div>

      {/* Stats Section */}
      <div className="absolute bottom-16 left-0 right-0 px-4">
        <FrogStats
          jump={frog.derived?.jump}
          vibe={vibeEntry(frog.derived?.temperament)}
          speed={frog.derived?.speed}
          intelligence={frog.derived?.intelligence}
          beauty={frog.derived?.beauty}
        />
      </div>

      {/* See more link */}
      <div className="absolute bottom-4 left-0 right-0 text-center">
        <button 
          className="text-teal-600 text-sm font-medium hover:text-teal-700 transition-colors"
          onClick={(e) => {
            e.stopPropagation();
            setExpanded(!expanded);
          }}
        >
          See more
        </button>
      </div>

      {/* Level up section */}
      {frog.offer_level_up && (
        <div className="absolute bottom-0 left-0 right-0 p-2 bg-gray-50 rounded-b-xl">
          <Button
            className="w-full bg-green-600 hover:bg-green-700 text-white text-sm"
            onClick={(e) => {
              e.stopPropagation();
              // Handle level up
            }}
          >
            Level up
          </Button>
        </div>
      )}

      {/* Expanded description */}
      {expanded && haveDesc && frog.derived?.description && (
        <div className="absolute inset-0 bg-white rounded-xl p-4 z-10">
          <div className="h-full overflow-y-auto">
            <div 
              className="text-gray-800 text-sm"
              style={{
                fontFamily: 'SF Pro Display',
                fontSize: '14px',
                color: '#293231'
              }}
            >
              {frog.derived.description}
            </div>
            <button 
              className="absolute top-2 right-2 text-gray-500 hover:text-gray-700"
              onClick={(e) => {
                e.stopPropagation();
                setExpanded(false);
              }}
            >
              ✕
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
