import { cn } from "@/lib/utils";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { SidebarIcon } from "lucide-react";
import { useTopBar } from "./TopBarContext";
import { Button } from "./ui/button";
import { useSidebar } from "./ui/sidebar";

export function TopBar() {
  const { toggleSidebar, state } = useSidebar();
  const { slots } = useTopBar();
  const handleMouseDown = (e: React.MouseEvent<HTMLDivElement>) => {
    // Check if the clicked element is an interactive control
    const target = e.target as HTMLElement;
    const isInteractive = target.closest(
      'button, input, select, textarea, label, [role="button"], [role="switch"], [tabindex]:not([tabindex="-1"])'
    );

    if (e.buttons === 1 && !isInteractive) {
      // Primary (left) button and not clicking on an interactive element
      e.detail === 2
        ? getCurrentWindow().toggleMaximize() // Maximize on double click
        : getCurrentWindow().startDragging(); // Else start dragging
    }
  };
  return (
    <div
      onMouseDown={handleMouseDown}
      className="fixed top-0 left-0 right-0 h-(--top-bar-height) bg-background flex w-full z-50 border-b border-border"
    >
      <div
        className={cn(
          "w-(--sidebar-width) h-full bg-background flex items-center pr-2",
          state === "collapsed" ? "w-28" : "w-(--sidebar-width)"
        )}
      >
        <div className="flex flex-col items-end w-full">
          <Button variant="ghost" size="icon" onClick={toggleSidebar}>
            <SidebarIcon />
          </Button>
        </div>
      </div>
      <div className="flex flex-1 pl-2 pr-4 items-center justify-between">
        <div className="flex items-center">{slots.left}</div>
        <div className="flex items-center">{slots.center}</div>
        <div className="flex items-center">{slots.right}</div>
      </div>
    </div>
  );
}
