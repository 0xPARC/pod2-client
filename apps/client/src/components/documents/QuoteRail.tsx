import { useEffect, useRef, useState } from "react";
import { MessageSquareIcon } from "lucide-react";
import { Button } from "../ui/button";
import { getBlockPositions, type BlockPosition } from "../../lib/blockUtils";

interface QuoteRailProps {
  contentRef: React.RefObject<HTMLElement | null>;
  selectedBlocks: Set<number>;
  onBlockToggle: (blockIndex: number, shiftKey?: boolean) => void;
  onQuoteSelected: () => void;
  className?: string;
}

export function QuoteRail({
  contentRef,
  selectedBlocks,
  onBlockToggle,
  onQuoteSelected,
  className = ""
}: QuoteRailProps) {
  const railRef = useRef<HTMLDivElement>(null);
  const [blockPositions, setBlockPositions] = useState<BlockPosition[]>([]);
  const [isVisible, setIsVisible] = useState(false);

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
    onBlockToggle(blockIndex, event.shiftKey);
  };

  return (
    <div
      ref={railRef}
      className={`absolute left-0 w-12 h-full bg-background/80 backdrop-blur-sm border-r transition-opacity duration-200 z-20 ${
        isVisible || selectedBlocks.size > 0 ? "opacity-100" : "opacity-0"
      } ${className}`}
    >
      {/* Rail hint */}
      <div className="absolute top-4 left-0 right-0 text-center">
        <div className="text-xs text-muted-foreground font-medium rotate-90 origin-center whitespace-nowrap">
          Select to quote
        </div>
      </div>

      {/* Block nodes */}
      <div className="relative h-full">
        {blockPositions.map(({ index, top, height }) => (
          <button
            key={index}
            className={`absolute left-3 w-4 h-4 rounded-full border-2 transition-all duration-150 hover:scale-110 ${
              selectedBlocks.has(index)
                ? "bg-blue-500 border-blue-600 shadow-sm"
                : "bg-muted border-border hover:border-blue-300 hover:bg-blue-50"
            } ${!isVisible && selectedBlocks.size === 0 ? "pointer-events-none" : ""}`}
            style={{ top: `${top + height / 2 - 8}px` }}
            onClick={(e) => handleBlockClick(index, e)}
            title={`Block ${index + 1}${selectedBlocks.has(index) ? " (selected)" : ""}`}
          />
        ))}
      </div>

      {/* Quote button */}
      {selectedBlocks.size > 0 && (
        <div className="absolute bottom-4 left-1 right-1">
          <Button
            onClick={onQuoteSelected}
            size="sm"
            className={`w-10 h-10 p-0 rounded-full bg-blue-600 hover:bg-blue-700 shadow-lg ${!isVisible && selectedBlocks.size === 0 ? "pointer-events-none" : ""}`}
            title={`Quote ${selectedBlocks.size} block${selectedBlocks.size > 1 ? "s" : ""}`}
          >
            <MessageSquareIcon className="h-4 w-4" />
          </Button>
          <div className="text-xs text-center text-muted-foreground mt-1 font-medium">
            {selectedBlocks.size}
          </div>
        </div>
      )}
    </div>
  );
}
