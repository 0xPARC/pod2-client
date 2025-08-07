// Component for incremental markdown preview updates
import { useEffect, useRef, forwardRef } from "react";
import type { AffectedRegion } from "../../workers/markdown.worker";

interface IncrementalMarkdownPreviewProps {
  html: string;
  affectedRegions: AffectedRegion[];
  isIncrementalMode: boolean;
  className?: string;
}

export const IncrementalMarkdownPreview = forwardRef<
  HTMLDivElement,
  IncrementalMarkdownPreviewProps
>(({ html, affectedRegions, isIncrementalMode, className }, ref) => {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const lastHtmlRef = useRef<string>("");

  // Combine refs
  const setRef = (element: HTMLDivElement | null) => {
    containerRef.current = element;
    if (typeof ref === "function") {
      ref(element);
    } else if (ref) {
      ref.current = element;
    }
  };

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    // Check if we should use incremental updates
    const shouldUseIncremental =
      isIncrementalMode &&
      affectedRegions.length > 0 &&
      lastHtmlRef.current &&
      lastHtmlRef.current !== html;

    if (shouldUseIncremental) {
      // Perform incremental update (simplified for now)
      // In a full implementation, we would:
      // 1. Parse the new HTML
      // 2. Identify which DOM nodes correspond to affected regions
      // 3. Update only those nodes
      // 4. Preserve scroll position and selection

      // For now, fall back to full update but preserve scroll position
      const scrollTop = container.scrollTop;
      const scrollLeft = container.scrollLeft;

      container.innerHTML = html;

      // Restore scroll position
      container.scrollTop = scrollTop;
      container.scrollLeft = scrollLeft;

      // TODO: Implement true incremental DOM updates
      console.log(
        `Incremental update: ${affectedRegions.length} regions affected`,
        affectedRegions
      );
    } else {
      // Full update
      container.innerHTML = html;
    }

    lastHtmlRef.current = html;
  }, [html, affectedRegions, isIncrementalMode]);

  return <div ref={setRef} className={className} />;
});

IncrementalMarkdownPreview.displayName = "IncrementalMarkdownPreview";
