import { CustomToggle } from "./CustomToggle";
import { cn } from "@/lib/utils";

interface MiningToggleProps {
  enabled: boolean;
  onToggle: (enabled: boolean) => void;
  hashCount?: string;
  className?: string;
}

export function MiningToggle({ 
  enabled, 
  onToggle, 
  hashCount = "", 
  className 
}: MiningToggleProps) {
  return (
    <div className={cn("flex items-center gap-3", className)}>
      <CustomToggle
        enabled={enabled}
        onToggle={onToggle}
      />
      <span 
        className="text-white"
        style={{
          fontFamily: 'var(--font-sans)',
          fontSize: '16px',
          fontWeight: 500,
          letterSpacing: '0.5px',
          lineHeight: '1.4'
        }}
      >
        {enabled ? 'Mining enabled' : 'Mining disabled'} {hashCount && `• Searched ${hashCount}K hashes for frogs`}
      </span>
    </div>
  );
}
