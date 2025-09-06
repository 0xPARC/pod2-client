import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface NavigationTabsProps {
  activeTab: string;
  onTabChange: (tab: string) => void;
  className?: string;
}

const TABS = [
  { id: "game", label: "get frogs" },
  { id: "gym", label: "frog gym" },
  { id: "breeding", label: "breeding" },
  { id: "frogedex", label: "frogedex" },
  { id: "leaderboard", label: "hi scores" }
] as const;

export function NavigationTabs({ activeTab, onTabChange, className }: NavigationTabsProps) {
  return (
    <div className={cn("flex gap-4 justify-center", className)}>
      {TABS.map((tab) => (
        <Button
          key={tab.id}
          className={cn(
            "h-7 px-5 text-white font-medium text-base tracking-wide transition-all duration-200",
            activeTab === tab.id
              ? "bg-gray-500" // #7E8C8C equivalent
              : "hover:bg-gray-500" // Same hover color as active state
          )}
          onClick={() => onTabChange(tab.id)}
          style={{
            fontFamily: 'var(--font-sans)',
            fontSize: '16px',
            fontWeight: 500,
            letterSpacing: '0.32px',
            width: '110px',
            height: '28px',
            backgroundColor: activeTab === tab.id ? undefined : '#325F58'
          }}
        >
          {tab.label}
        </Button>
      ))}
    </div>
  );
}
