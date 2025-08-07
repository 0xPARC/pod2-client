// Component for incremental markdown preview updates with surgical DOM operations
import { useEffect, useRef, forwardRef } from "react";
import type {
  AffectedRegion,
  BlockMapping
} from "../../workers/markdown.worker";

interface IncrementalMarkdownPreviewProps {
  html: string;
  affectedRegions: AffectedRegion[];
  blockMappings: BlockMapping[];
  isIncrementalMode: boolean;
  className?: string;
}

// Helper function to parse HTML string into DOM tree
function parseHTMLToDOM(htmlString: string): DocumentFragment {
  const template = document.createElement("template");
  template.innerHTML = htmlString.trim();
  return template.content;
}

// Helper function to find elements that correspond to affected regions
function findAffectedElements(
  container: HTMLDivElement,
  affectedRegions: AffectedRegion[],
  blockMappings: BlockMapping[]
): { element: Element; region: AffectedRegion; mapping: BlockMapping }[] {
  const results: {
    element: Element;
    region: AffectedRegion;
    mapping: BlockMapping;
  }[] = [];

  // Create a map of line ranges to block mappings for efficient lookup
  const lineToMapping = new Map<number, BlockMapping>();
  blockMappings.forEach((mapping) => {
    for (let line = mapping.startLine; line <= mapping.endLine; line++) {
      lineToMapping.set(line, mapping);
    }
  });

  affectedRegions.forEach((region) => {
    // Find block mappings that overlap with this affected region
    for (let line = region.startLine; line <= region.endLine; line++) {
      const mapping = lineToMapping.get(line);
      if (mapping) {
        // Find the DOM element with the corresponding element index
        const element = container.querySelector(
          `[data-md-element-index="${mapping.elementIndex}"]`
        );
        if (element) {
          results.push({ element, region, mapping });
          break; // Don't duplicate the same element
        }
      }
    }
  });

  return results;
}

export const IncrementalMarkdownPreview = forwardRef<
  HTMLDivElement,
  IncrementalMarkdownPreviewProps
>(
  (
    { html, affectedRegions, blockMappings, isIncrementalMode, className },
    ref
  ) => {
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
        blockMappings.length > 0 &&
        lastHtmlRef.current &&
        lastHtmlRef.current !== html;

      if (shouldUseIncremental) {
        try {
          // Preserve scroll position only (no selection manipulation to avoid Monaco conflicts)
          const scrollTop = container.scrollTop;
          const scrollLeft = container.scrollLeft;

          // Parse the new HTML
          const newContent = parseHTMLToDOM(html);

          // Find existing elements that need to be replaced
          const affectedElements = findAffectedElements(
            container,
            affectedRegions,
            blockMappings
          );

          if (affectedElements.length > 0) {
            // Create a temporary container to hold new content
            const tempContainer = document.createElement("div");
            tempContainer.appendChild(newContent.cloneNode(true));

            // Replace each affected element using DOM-safe in-place updates
            affectedElements.forEach(({ element, mapping }) => {
              // Find the corresponding new element
              const newElement = tempContainer.querySelector(
                `[data-md-element-index="${mapping.elementIndex}"]`
              );

              if (newElement && element.parentNode) {
                // Instead of replaceChild (which Monaco might reference),
                // update the element in-place
                element.innerHTML = newElement.innerHTML;

                // Copy attributes
                Array.from(newElement.attributes).forEach((attr) => {
                  element.setAttribute(attr.name, attr.value);
                });
              }
            });

            // Restore scroll position
            container.scrollTop = scrollTop;
            container.scrollLeft = scrollLeft;
          } else {
            // Fallback to full update if we couldn't find specific elements
            container.innerHTML = html;
            container.scrollTop = scrollTop;
            container.scrollLeft = scrollLeft;
          }
        } catch (error) {
          // Fallback to full update on any error
          const scrollTop = container.scrollTop;
          const scrollLeft = container.scrollLeft;

          container.innerHTML = html;

          container.scrollTop = scrollTop;
          container.scrollLeft = scrollLeft;
        }
      } else {
        // Full update (first render or no incremental data)
        container.innerHTML = html;
      }

      lastHtmlRef.current = html;
    }, [html, affectedRegions, blockMappings, isIncrementalMode]);

    return <div ref={setRef} className={className} />;
  }
);

IncrementalMarkdownPreview.displayName = "IncrementalMarkdownPreview";
