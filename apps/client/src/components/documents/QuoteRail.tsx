import { MessageSquareQuote } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { getBlockPositions, type BlockPosition } from "../../lib/blockUtils";
import { useDocuments } from "../../lib/store";

interface QuoteRailProps {
  contentRef: React.RefObject<HTMLElement | null>;
  className?: string;
  mode?: "view" | "context";
}

export function QuoteRail({
  contentRef,
  className = "",
  mode = "view"
}: QuoteRailProps) {
  const railRef = useRef<HTMLDivElement>(null);
  const [blockPositions, setBlockPositions] = useState<BlockPosition[]>([]);
  const [isVisible, setIsVisible] = useState(false);

  // Get selection state and actions from store based on mode
  const {
    selectedBlockIndices,
    toggleBlockSelection,
    contextSelectedBlockIndices,
    toggleContextBlockSelection
  } = useDocuments();

  // Use different state/actions based on mode
  const activeSelectedIndices =
    mode === "view" ? selectedBlockIndices : contextSelectedBlockIndices;
  const activeToggleSelection =
    mode === "view" ? toggleBlockSelection : toggleContextBlockSelection;
  const selectedBlocks = new Set(activeSelectedIndices);

  // Update block positions when content changes
  useEffect(() => {
    if (!contentRef.current) return;

    const updatePositions = () => {
      const positions = getBlockPositions(contentRef.current!);
      setBlockPositions(positions);
    };

    updatePositions();

    // Update positions on scroll and resize
    const container = contentRef.current;
    const observer = new ResizeObserver(updatePositions);
    observer.observe(container);

    window.addEventListener("scroll", updatePositions);
    window.addEventListener("resize", updatePositions);

    return () => {
      observer.disconnect();
      window.removeEventListener("scroll", updatePositions);
      window.removeEventListener("resize", updatePositions);
    };
  }, [contentRef]);

  // Show/hide rail on hover
  useEffect(() => {
    if (!contentRef.current || !railRef.current) return;

    const container = contentRef.current;
    const rail = railRef.current;

    const handleMouseEnter = () => setIsVisible(true);

    // Only hide when mouse leaves both the container AND rail areas
    const handleMouseLeave = (e: MouseEvent) => {
      const relatedTarget = e.relatedTarget as Element | null;

      // Don't hide if mouse is moving to the rail from content or vice versa
      if (
        relatedTarget &&
        (container.contains(relatedTarget) ||
          rail.contains(relatedTarget) ||
          relatedTarget === container ||
          relatedTarget === rail)
      ) {
        return;
      }

      setIsVisible(false);
    };

    container.addEventListener("mouseenter", handleMouseEnter);
    container.addEventListener("mouseleave", handleMouseLeave);
    rail.addEventListener("mouseenter", handleMouseEnter);
    rail.addEventListener("mouseleave", handleMouseLeave);

    return () => {
      container.removeEventListener("mouseenter", handleMouseEnter);
      container.removeEventListener("mouseleave", handleMouseLeave);
      rail.removeEventListener("mouseenter", handleMouseEnter);
      rail.removeEventListener("mouseleave", handleMouseLeave);
    };
  }, [contentRef]);

  const handleBlockClick = (blockIndex: number, event: React.MouseEvent) => {
    // Just toggle the block selection using store action
    activeToggleSelection(blockIndex, event.shiftKey);
  };

  return (
    <div
      ref={railRef}
      className={`absolute left-0 w-12 h-full bg-background/80 backdrop-blur-sm transition-opacity duration-200 z-20 ${
        isVisible || selectedBlocks.size > 0 ? "opacity-100" : "opacity-0"
      } ${className}`}
    >
      {/* Rail hint */}
      <div className="absolute top-2 left-0 right-3 text-center">
        <div className="text-xs text-muted-foreground font-medium">Quote</div>
      </div>

      {/* Block nodes */}
      <div className="relative h-full">
        {blockPositions.map(({ index, top, height }) => (
          <button
            key={index}
            className={`absolute left-2 w-5 h-5 flex items-center justify-center transition-all duration-150 hover:scale-110 ${
              selectedBlocks.has(index)
                ? "text-blue-600 opacity-100"
                : "text-muted-foreground opacity-30 hover:opacity-100 hover:text-blue-500"
            } ${!isVisible && selectedBlocks.size === 0 ? "pointer-events-none" : ""}`}
            style={{ top: `${top + height / 2 - 10}px` }}
            onClick={(e) => handleBlockClick(index, e)}
            title={`Block ${index + 1}${selectedBlocks.has(index) ? " (selected)" : ""}`}
          >
            <MessageSquareQuote className="h-4 w-4" />
          </button>
        ))}
      </div>
    </div>
  );
}
