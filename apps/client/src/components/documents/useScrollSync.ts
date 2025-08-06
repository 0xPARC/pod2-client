// Hook for managing scroll synchronization between Monaco editor and markdown preview
import { useCallback, useEffect, useRef, useState } from "react";
import type * as monaco from "monaco-editor/esm/vs/editor/editor.api";

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

  // Enable/disable sync in specific directions
  enableSync: (enabled: boolean) => void;

  // Current block geometries (for debugging)
  blockGeometries: BlockGeometry[];
}

export function useScrollSync(
  options: UseScrollSyncOptions = {}
): UseScrollSyncResult {
  const { cooldownMs = 100 } = options;

  // Refs for editor and preview
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const previewRef = useRef<HTMLElement | null>(null);

  // State for block geometries
  const [blockGeometries, setBlockGeometries] = useState<BlockGeometry[]>([]);

  // Sync control
  const [syncEnabled, setSyncEnabled] = useState(true);
  const syncingFromEditor = useRef(false);
  const syncingFromPreview = useRef(false);
  const lastSyncTime = useRef(0);
  const lastSyncDirection = useRef<"editor" | "preview" | null>(null);

  // Sync timing control
  const syncCooldownTimer = useRef<NodeJS.Timeout | undefined>(undefined);

  // Update block geometry calculations
  const updateBlockGeometries = useCallback(
    (mappings: BlockMapping[]) => {
      const editor = editorRef.current;
      const preview = previewRef.current;

      if (!editor || !preview || !syncEnabled) {
        setBlockGeometries([]);
        return;
      }

      // Skip if in cooldown period
      if (Date.now() - lastSyncTime.current < cooldownMs) {
        return;
      }

      const newGeometries: BlockGeometry[] = [];

      for (const mapping of mappings) {
        // Find the corresponding HTML element
        const element = preview.querySelector(
          `[data-md-element-index="${mapping.elementIndex}"]`
        ) as HTMLElement;

        if (element) {
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

      setBlockGeometries(newGeometries);
    },
    [syncEnabled]
  );

  // Find block containing a specific line
  const findBlockForLine = useCallback(
    (lineNumber: number): BlockGeometry | null => {
      return (
        blockGeometries.find(
          (geom) =>
            lineNumber >= geom.mapping.startLine &&
            lineNumber <= geom.mapping.endLine
        ) || null
      );
    },
    [blockGeometries]
  );

  // Find block containing a specific scroll position
  const findBlockForScrollTop = useCallback(
    (scrollTop: number): BlockGeometry | null => {
      return (
        blockGeometries.find(
          (geom) =>
            scrollTop >= geom.offsetTop &&
            scrollTop < geom.offsetTop + geom.offsetHeight
        ) || null
      );
    },
    [blockGeometries]
  );

  // Sync editor scroll to preview
  const syncEditorToPreview = useCallback(() => {
    const editor = editorRef.current;
    const preview = previewRef.current;

    if (!editor || !preview || !syncEnabled || syncingFromPreview.current) {
      return;
    }

    syncingFromEditor.current = true;

    try {
      // Get top visible line in editor
      const visibleRanges = editor.getVisibleRanges();
      if (visibleRanges.length === 0) return;

      const topVisibleLine = visibleRanges[0].startLineNumber - 1; // Convert to 0-indexed
      // Get editor scroll position for diagnostics
      // const editorScrollTop = editor.getScrollTop();

      // Find the block containing this line
      const block = findBlockForLine(topVisibleLine);
      if (!block) return;

      // Calculate proportional position within the block
      const linePixelTop = editor.getTopForLineNumber(topVisibleLine + 1);
      const blockStartPixel = block.editorStartPixel;
      const blockEndPixel = block.editorEndPixel;

      const blockPixelRange = blockEndPixel - blockStartPixel;
      const proportion =
        blockPixelRange > 0
          ? Math.max(
              0,
              Math.min(1, (linePixelTop - blockStartPixel) / blockPixelRange)
            )
          : 0;

      // Apply same proportion to HTML element
      const targetScrollTop = block.offsetTop + proportion * block.offsetHeight;

      // Scroll preview
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
  }, [syncEnabled, cooldownMs, findBlockForLine]);

  // Sync preview scroll to editor
  const syncPreviewToEditor = useCallback(() => {
    const editor = editorRef.current;
    const preview = previewRef.current;

    if (!editor || !preview || !syncEnabled || syncingFromEditor.current) {
      return;
    }

    syncingFromPreview.current = true;

    try {
      const previewScrollTop = preview.scrollTop;

      // Find the block containing this scroll position
      const block = findBlockForScrollTop(previewScrollTop);
      if (!block) return;

      // Calculate proportional position within the HTML element
      const elementScrollOffset = previewScrollTop - block.offsetTop;
      const proportion =
        block.offsetHeight > 0
          ? Math.max(0, Math.min(1, elementScrollOffset / block.offsetHeight))
          : 0;

      // Apply same proportion to editor block
      // const blockPixelRange = block.editorEndPixel - block.editorStartPixel;
      // Calculate target pixel position for diagnostics
      // const targetPixelTop = block.editorStartPixel + (proportion * blockPixelRange);

      // Convert pixel position back to line number (approximately)
      const targetLineNumber = Math.max(
        1,
        Math.round(
          block.mapping.startLine +
            1 +
            proportion * (block.mapping.endLine - block.mapping.startLine)
        )
      );

      // Reveal the line in editor
      editor.revealLineNearTop(targetLineNumber);
    } finally {
      // Record sync time and direction
      lastSyncTime.current = Date.now();
      lastSyncDirection.current = "preview";

      // Clear sync flag after cooldown period
      setTimeout(() => {
        syncingFromPreview.current = false;
      }, cooldownMs);
    }
  }, [syncEnabled, cooldownMs, findBlockForScrollTop]);

  // Direct scroll handlers without debouncing
  const handleEditorScroll = useCallback(() => {
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

    // Sync immediately without debouncing
    syncPreviewToEditor();
  }, [syncPreviewToEditor, cooldownMs]);

  // Set up scroll event listeners
  useEffect(() => {
    const editor = editorRef.current;
    const preview = previewRef.current;

    if (!editor || !preview || !syncEnabled) {
      return;
    }

    // Listen to editor scroll events
    const editorScrollDisposable = editor.onDidScrollChange(handleEditorScroll);

    // Listen to preview scroll events
    preview.addEventListener("scroll", handlePreviewScroll, { passive: true });

    return () => {
      editorScrollDisposable.dispose();
      preview.removeEventListener("scroll", handlePreviewScroll);

      // Clear any pending cooldown timer
      if (syncCooldownTimer.current) {
        clearTimeout(syncCooldownTimer.current);
      }
    };
  }, [syncEnabled, handleEditorScroll, handlePreviewScroll]);

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

  const enableSync = useCallback((enabled: boolean) => {
    setSyncEnabled(enabled);
  }, []);

  return {
    setEditorRef,
    setPreviewRef,
    updateBlockMappings,
    enableSync,
    blockGeometries
  };
}
