// Hook for managing scroll synchronization between Monaco editor and markdown preview
import type * as monaco from "monaco-editor/esm/vs/editor/editor.api";
import { useCallback, useEffect, useRef, useState } from "react";

// Default scroll sync cooldown to prevent editor/preview feedback loops
const DEFAULT_SCROLL_SYNC_COOLDOWN_MS = 100;

export interface BlockMapping {
  startLine: number;
  endLine: number;
  elementType: string;
  elementIndex: number;
}

export interface BlockGeometry {
  element: HTMLElement;
  mapping: BlockMapping;
  offsetTop: number;
  offsetHeight: number;
  editorStartPixel: number;
  editorEndPixel: number;
}

interface UseScrollSyncOptions {
  // Sync cooldown period to prevent feedback loops (in milliseconds)
  cooldownMs?: number;
}

interface UseScrollSyncResult {
  // Set references to the editor and preview container
  setEditorRef: (editor: monaco.editor.IStandaloneCodeEditor | null) => void;
  setPreviewRef: (container: HTMLElement | null) => void;

  // Update block mappings when they change
  updateBlockMappings: (mappings: BlockMapping[]) => void;

  // Manual sync triggers for view mode transitions
  syncEditorToPreview: () => void;
  syncPreviewToEditor: () => void;

  // Layout change control
  setLayoutChanging: (changing: boolean) => void;

  // Current block geometries (for debugging)
  blockGeometries: BlockGeometry[];
}

export function useScrollSync(
  options: UseScrollSyncOptions = {}
): UseScrollSyncResult {
  const { cooldownMs = DEFAULT_SCROLL_SYNC_COOLDOWN_MS } = options;

  // Refs for editor and preview
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const previewRef = useRef<HTMLElement | null>(null);

  // State for block geometries
  const [blockGeometries, setBlockGeometries] = useState<BlockGeometry[]>([]);

  // Store current mappings in a ref to avoid recreating arrays
  const currentMappings = useRef<BlockMapping[]>([]);

  // Scroll sync is always enabled
  const syncingFromEditor = useRef(false);
  const syncingFromPreview = useRef(false);
  const lastSyncTime = useRef(0);
  const lastSyncDirection = useRef<"editor" | "preview" | null>(null);

  // Sync timing control
  const syncCooldownTimer = useRef<NodeJS.Timeout | undefined>(undefined);
  const isLayoutChanging = useRef(false);

  // Update block geometry calculations
  const updateBlockGeometries = useCallback(
    (mappings: BlockMapping[]) => {
      const editor = editorRef.current;
      const preview = previewRef.current;

      if (!editor || !preview) {
        setBlockGeometries([]);
        return;
      }

      // Skip if in cooldown period
      if (Date.now() - lastSyncTime.current < cooldownMs) {
        return;
      }

      // Store current mappings for use in resize handler
      currentMappings.current = mappings;

      const newGeometries: BlockGeometry[] = [];

      for (const mapping of mappings) {
        // Find the corresponding HTML element using element index
        let element = preview.querySelector(
          `[data-md-element-index="${mapping.elementIndex}"]`
        ) as HTMLElement;

        // If no direct element found, look for element by line start (fallback)
        if (!element) {
          element = preview.querySelector(
            `[data-md-line-start="${mapping.startLine}"]`
          ) as HTMLElement;
        }

        if (element) {
          // For MathJax containers, use the first mjx-container for more accurate dimensions
          const mathContainer = element.querySelector(
            "mjx-container"
          ) as HTMLElement;
          if (mathContainer) {
            element = mathContainer;
          }

          // Get HTML element geometry
          const rect = element.getBoundingClientRect();
          const previewRect = preview.getBoundingClientRect();
          const offsetTop = rect.top - previewRect.top + preview.scrollTop;
          const offsetHeight = rect.height;

          // Get editor line positions
          const editorStartPixel = editor.getTopForLineNumber(
            mapping.startLine + 1
          ); // Monaco is 1-indexed
          const editorEndPixel = editor.getTopForLineNumber(
            mapping.endLine + 2
          ); // +2 for exclusive end

          newGeometries.push({
            element,
            mapping,
            offsetTop,
            offsetHeight,
            editorStartPixel,
            editorEndPixel
          });
        }
      }

      // Note: MathJax containers are now properly wrapped with line mapping attributes

      setBlockGeometries(newGeometries);
    },
    [cooldownMs]
  );

  // Note: findBlockForLine removed - we now use direct scroll position matching

  // Note: findBlockForScrollTop removed - we now use viewport-center-based matching

  // Sync editor scroll to preview
  const syncEditorToPreview = useCallback(() => {
    const editor = editorRef.current;
    const preview = previewRef.current;

    if (!editor || !preview || syncingFromPreview.current) {
      return;
    }

    syncingFromEditor.current = true;

    try {
      // Get current editor scroll position
      const editorScrollTop = editor.getScrollTop();

      // Find which block contains this scroll position
      let targetBlock: BlockGeometry | null = null;
      let scrollWithinBlock = 0;

      for (const block of blockGeometries) {
        if (
          editorScrollTop >= block.editorStartPixel &&
          editorScrollTop <= block.editorEndPixel
        ) {
          targetBlock = block;
          scrollWithinBlock = editorScrollTop - block.editorStartPixel;
          break;
        }
        // Check if scroll position is between blocks (use the next block)
        if (editorScrollTop < block.editorStartPixel) {
          targetBlock = block;
          scrollWithinBlock = 0; // At the start of this block
          break;
        }
      }

      // If no block found, use the last block
      if (!targetBlock && blockGeometries.length > 0) {
        targetBlock = blockGeometries[blockGeometries.length - 1];
        const lastBlockRange =
          targetBlock.editorEndPixel - targetBlock.editorStartPixel;
        scrollWithinBlock = Math.max(
          0,
          editorScrollTop - targetBlock.editorStartPixel
        );
        scrollWithinBlock = Math.min(scrollWithinBlock, lastBlockRange);
      }

      if (!targetBlock) return;

      // Calculate proportional position within the editor block
      const blockPixelRange =
        targetBlock.editorEndPixel - targetBlock.editorStartPixel;
      const proportion =
        blockPixelRange > 0
          ? Math.max(0, Math.min(1, scrollWithinBlock / blockPixelRange))
          : 0;

      // Apply same proportion to HTML element
      const targetScrollTop =
        targetBlock.offsetTop + proportion * targetBlock.offsetHeight;

      // Scroll preview to maintain the same proportional position
      preview.scrollTo({
        top: targetScrollTop,
        behavior: "instant" // Use instant to avoid interfering with sync
      });
    } finally {
      // Record sync time and direction
      lastSyncTime.current = Date.now();
      lastSyncDirection.current = "editor";

      // Clear sync flag after cooldown period
      setTimeout(() => {
        syncingFromEditor.current = false;
      }, cooldownMs);
    }
  }, [cooldownMs, blockGeometries]);

  // Sync preview scroll to editor
  const syncPreviewToEditor = useCallback(() => {
    const editor = editorRef.current;
    const preview = previewRef.current;

    if (!editor || !preview || syncingFromEditor.current) {
      return;
    }

    syncingFromPreview.current = true;

    try {
      // Get preview viewport info
      const previewScrollTop = preview.scrollTop;
      const previewViewportHeight = preview.clientHeight;

      // Calculate preview viewport center for more stable syncing
      const previewViewportCenter =
        previewScrollTop + previewViewportHeight / 2;

      // Find the block containing the preview viewport center
      let targetBlock: BlockGeometry | null = null;
      let proportionWithinBlock = 0;

      for (const block of blockGeometries) {
        if (
          previewViewportCenter >= block.offsetTop &&
          previewViewportCenter <= block.offsetTop + block.offsetHeight
        ) {
          targetBlock = block;
          const elementScrollOffset = previewViewportCenter - block.offsetTop;
          proportionWithinBlock =
            block.offsetHeight > 0
              ? Math.max(
                  0,
                  Math.min(1, elementScrollOffset / block.offsetHeight)
                )
              : 0;
          break;
        }
        // Check if viewport center is between blocks (use the next block)
        if (previewViewportCenter < block.offsetTop) {
          targetBlock = block;
          proportionWithinBlock = 0; // At the start of this block
          break;
        }
      }

      // If no block found, use the last block
      if (!targetBlock && blockGeometries.length > 0) {
        targetBlock = blockGeometries[blockGeometries.length - 1];
        const elementScrollOffset = Math.max(
          0,
          previewViewportCenter - targetBlock.offsetTop
        );
        proportionWithinBlock =
          targetBlock.offsetHeight > 0
            ? Math.max(
                0,
                Math.min(1, elementScrollOffset / targetBlock.offsetHeight)
              )
            : 0;
      }

      if (!targetBlock) return;

      // Calculate corresponding position in editor and center it in editor viewport
      const blockPixelRange =
        targetBlock.editorEndPixel - targetBlock.editorStartPixel;
      const targetElementPosition =
        targetBlock.editorStartPixel + proportionWithinBlock * blockPixelRange;

      // Center this position in the editor viewport
      const editorViewportHeight = editor.getLayoutInfo().height;
      const targetEditorScrollTop =
        targetElementPosition - editorViewportHeight / 2;

      // Scroll editor to center the corresponding portion
      editor.setScrollTop(Math.max(0, targetEditorScrollTop)); // Don't scroll above content
    } finally {
      // Record sync time and direction
      lastSyncTime.current = Date.now();
      lastSyncDirection.current = "preview";

      // Clear sync flag after cooldown period
      setTimeout(() => {
        syncingFromPreview.current = false;
      }, cooldownMs);
    }
  }, [cooldownMs, blockGeometries]);

  // Direct scroll handlers without debouncing
  const handleEditorScroll = useCallback(() => {
    // Skip if layout is changing (geometry is stale)
    if (isLayoutChanging.current) {
      return;
    }

    // Skip if we recently synced from preview to editor
    const timeSinceLastSync = Date.now() - lastSyncTime.current;
    if (
      lastSyncDirection.current === "preview" &&
      timeSinceLastSync < cooldownMs
    ) {
      return;
    }

    // Skip if currently syncing from preview
    if (syncingFromPreview.current) {
      return;
    }

    // Sync immediately without debouncing
    syncEditorToPreview();
  }, [syncEditorToPreview, cooldownMs]);

  const handlePreviewScroll = useCallback(() => {
    // Skip if layout is changing (geometry is stale)
    if (isLayoutChanging.current) {
      console.log("🛑 Blocked preview scroll sync - layout changing");
      return;
    }

    // Skip if we recently synced from editor to preview
    const timeSinceLastSync = Date.now() - lastSyncTime.current;
    if (
      lastSyncDirection.current === "editor" &&
      timeSinceLastSync < cooldownMs
    ) {
      return;
    }

    // Skip if currently syncing from editor
    if (syncingFromEditor.current) {
      return;
    }

    console.log("🔄 Running preview scroll sync");
    // Sync immediately without debouncing
    syncPreviewToEditor();
  }, [syncPreviewToEditor, cooldownMs]);

  // Set up scroll event listeners and MathJax handling
  useEffect(() => {
    const editor = editorRef.current;
    const preview = previewRef.current;

    if (!editor || !preview) {
      return;
    }

    // Listen to editor scroll events
    const editorScrollDisposable = editor.onDidScrollChange(handleEditorScroll);

    // Listen to preview scroll events
    preview.addEventListener("scroll", handlePreviewScroll, { passive: true });

    // Set up ResizeObserver to detect preview container size changes (view mode transitions, window resizes)
    const handlePreviewResize = () => {
      // Mark that layout is changing to prevent scroll sync with stale geometry
      console.log("📏 ResizeObserver fired - blocking scroll sync");
      isLayoutChanging.current = true;

      // Delay geometry refresh to ensure layout has settled after resize
      setTimeout(() => {
        console.log("📏 Updating block geometries after resize");
        // Use stored current mappings instead of extracting from stale blockGeometries
        updateBlockGeometries(currentMappings.current);

        // Re-enable scroll sync after geometry is updated
        setTimeout(() => {
          console.log("✅ Re-enabling scroll sync after layout");
          isLayoutChanging.current = false;
        }, 50);
      }, 100);
    };

    const resizeObserver = new ResizeObserver(handlePreviewResize);
    resizeObserver.observe(preview);

    // To ensure geometry is updated after MathJax renders, we use multiple strategies:
    // 1. Listen for specific MathJax events for immediate feedback.
    // 2. Listen to DOMContentLoaded as a fallback for the initial page load.
    // 3. Use a MutationObserver to catch cases where MathJax modifies the DOM
    //    without firing a specific, catchable event. This is the most reliable
    //    fallback.

    // Listen for MathJax rendering completion to refresh geometry
    const handleMathJaxRendered = () => {
      // Delay geometry refresh to ensure MathJax is fully rendered
      setTimeout(() => {
        // Use stored current mappings instead of extracting from stale blockGeometries
        updateBlockGeometries(currentMappings.current);
      }, 150);
    };

    // Multiple event listeners for different MathJax events
    const mathJaxEvents = [
      "mjx-math-rendered", // MathJax v3 math rendered
      "mjx-startup-ready", // MathJax startup complete
      "mjx-typeset-complete", // Typesetting complete
      "DOMContentLoaded" // Fallback for initial render
    ];

    mathJaxEvents.forEach((eventName) => {
      preview.addEventListener(eventName, handleMathJaxRendered);
      document.addEventListener(eventName, handleMathJaxRendered);
    });

    // Listen for MutationObserver changes to detect MathJax rendering completion
    const mathJaxObserver = new MutationObserver((mutations) => {
      let hasMathJaxChanges = false;

      mutations.forEach((mutation) => {
        if (mutation.type === "childList") {
          const addedNodes = Array.from(mutation.addedNodes);

          addedNodes.forEach((node) => {
            if (node instanceof Element) {
              // Check if this is a MathJax container or contains one
              const mathContainers =
                node.tagName === "MJX-CONTAINER"
                  ? [node as HTMLElement]
                  : (Array.from(
                      node.querySelectorAll("mjx-container")
                    ) as HTMLElement[]);

              if (mathContainers.length > 0) {
                hasMathJaxChanges = true;
              }
            }
          });
        }
      });

      if (hasMathJaxChanges) {
        handleMathJaxRendered();
      }
    });

    mathJaxObserver.observe(preview, {
      childList: true,
      subtree: true
    });

    return () => {
      editorScrollDisposable.dispose();
      preview.removeEventListener("scroll", handlePreviewScroll);

      // Disconnect ResizeObserver
      resizeObserver.disconnect();

      // Remove all MathJax event listeners
      mathJaxEvents.forEach((eventName) => {
        preview.removeEventListener(eventName, handleMathJaxRendered);
        document.removeEventListener(eventName, handleMathJaxRendered);
      });

      // Disconnect MutationObserver
      mathJaxObserver.disconnect();

      // Clear any pending cooldown timer
      if (syncCooldownTimer.current) {
        clearTimeout(syncCooldownTimer.current);
      }
    };
  }, [handleEditorScroll, handlePreviewScroll, updateBlockGeometries]);

  // Public API
  const setEditorRef = useCallback(
    (editor: monaco.editor.IStandaloneCodeEditor | null) => {
      editorRef.current = editor;
    },
    []
  );

  const setPreviewRef = useCallback((container: HTMLElement | null) => {
    previewRef.current = container;
  }, []);

  const updateBlockMappings = useCallback(
    (mappings: BlockMapping[]) => {
      updateBlockGeometries(mappings);
    },
    [updateBlockGeometries]
  );

  const setLayoutChanging = useCallback((changing: boolean) => {
    isLayoutChanging.current = changing;
    if (changing) {
      console.log("🛑 Layout changing - blocking scroll sync");
    } else {
      console.log("✅ Layout settled - enabling scroll sync");
    }
  }, []);

  return {
    setEditorRef,
    setPreviewRef,
    updateBlockMappings,
    syncEditorToPreview,
    syncPreviewToEditor,
    setLayoutChanging,
    blockGeometries
  };
}
