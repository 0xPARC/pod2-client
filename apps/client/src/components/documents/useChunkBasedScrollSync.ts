import { useCallback, useRef, useEffect } from "react";

interface ChunkBasedScrollSyncOptions {
  editScrollThrottle?: number;
  viewScrollThrottle?: number;
}

interface MarkdownChunk {
  id: string;
  startLine: number;
  endLine: number;
  visualStartLine: number;
  visualEndLine: number;
  content: string;
  renderedHtml: string;
  hash: string;
}

export function useChunkBasedScrollSync(
  chunks: MarkdownChunk[],
  options: ChunkBasedScrollSyncOptions = {}
) {
  const {
    editScrollThrottle = 16, // ~60fps
    viewScrollThrottle = 16
  } = options;

  const editAreaRef = useRef<HTMLTextAreaElement>(null);
  const viewAreaRef = useRef<HTMLDivElement>(null);
  const viewScrollingRef = useRef(false);
  const editScrollingRef = useRef(false);
  const viewScrollingTimerRef = useRef<NodeJS.Timeout | null>(null);
  const editScrollingTimerRef = useRef<NodeJS.Timeout | null>(null);

  // Find which chunk contains a given visual line number
  const findChunkForVisualLine = useCallback(
    (visualLine: number): { chunk: MarkdownChunk; element: Element } | null => {
      const viewArea = viewAreaRef.current;
      if (!viewArea) return null;

      // Find chunk data that contains this visual line
      const chunkData = chunks.find(
        (chunk) =>
          visualLine >= chunk.visualStartLine &&
          visualLine <= chunk.visualEndLine
      );

      if (!chunkData) return null;

      // Find corresponding DOM element
      const elements = viewArea.querySelectorAll(
        ".part[data-startline][data-endline]"
      );

      for (const element of elements) {
        const startLine = parseInt(
          element.getAttribute("data-startline") || "0",
          10
        );
        const endLine = parseInt(
          element.getAttribute("data-endline") || "0",
          10
        );

        if (
          startLine === chunkData.startLine + 1 &&
          endLine === chunkData.endLine + 1
        ) {
          return { chunk: chunkData, element };
        }
      }

      return null;
    },
    [chunks]
  );

  // Get the chunk that's currently at the top of the preview viewport
  const getCurrentViewChunk = useCallback((): Element | null => {
    const viewArea = viewAreaRef.current;
    if (!viewArea) return null;

    const chunks = viewArea.querySelectorAll(".part[data-startline]");
    const scrollTop = viewArea.scrollTop;

    // Find the first chunk that's visible (its bottom edge is below the scroll position)
    for (const chunk of chunks) {
      const rect = chunk.getBoundingClientRect();
      const containerRect = viewArea.getBoundingClientRect();
      const relativeTop = rect.top - containerRect.top + scrollTop;
      const relativeBottom = relativeTop + rect.height;

      if (relativeBottom > scrollTop) {
        return chunk;
      }
    }

    return null;
  }, []);

  // Sync scroll from editor to preview using visual line mapping
  const syncScrollToView = useCallback(() => {
    const editArea = editAreaRef.current;
    const viewArea = viewAreaRef.current;

    if (!editArea || !viewArea || viewScrollingRef.current) {
      return;
    }

    const lineHeight = parseFloat(getComputedStyle(editArea).lineHeight) || 20;
    const editorScrollTop = editArea.scrollTop;

    // Find the visual line at the very top of the editor viewport
    const topVisualLine = Math.floor(editorScrollTop / lineHeight);
    const chunkInfo = findChunkForVisualLine(topVisualLine);

    if (!chunkInfo) {
      return;
    }

    const { chunk: chunkData, element: chunkElement } = chunkInfo;

    // How far are we into this chunk in the editor (in visual lines)?
    const visualLinesIntoChunk = topVisualLine - chunkData.visualStartLine;
    const editorChunkVisualHeight =
      chunkData.visualEndLine - chunkData.visualStartLine + 1;

    // What's the equivalent position in the preview chunk?
    const previewChunkRect = chunkElement.getBoundingClientRect();
    const previewChunkTop =
      previewChunkRect.top -
      viewArea.getBoundingClientRect().top +
      viewArea.scrollTop;
    const previewChunkHeight = previewChunkRect.height;

    // Apply proportional offset within the chunk
    const proportionalOffset =
      (visualLinesIntoChunk / editorChunkVisualHeight) * previewChunkHeight;
    const calculatedScrollTop = previewChunkTop + proportionalOffset;

    // Add 20px safety margin to stay slightly higher in preview
    const targetScrollTop = Math.max(0, calculatedScrollTop - 20);

    // Prevent reverse sync
    editScrollingRef.current = true;
    viewArea.scrollTop = targetScrollTop;

    if (editScrollingTimerRef.current) {
      clearTimeout(editScrollingTimerRef.current);
    }
    editScrollingTimerRef.current = setTimeout(() => {
      editScrollingRef.current = false;
    }, 50);
  }, [findChunkForVisualLine]);

  // Sync scroll from preview to editor using visual line mapping
  const syncScrollToEdit = useCallback(() => {
    const editArea = editAreaRef.current;
    const viewArea = viewAreaRef.current;

    if (!editArea || !viewArea || editScrollingRef.current) {
      return;
    }

    const viewScrollTop = viewArea.scrollTop;
    const topChunk = getCurrentViewChunk();

    if (!topChunk) {
      return;
    }

    // Find the chunk data that corresponds to this DOM element
    const chunkStartLine =
      parseInt(topChunk.getAttribute("data-startline") || "0", 10) - 1;
    const chunkEndLine =
      parseInt(topChunk.getAttribute("data-endline") || "0", 10) - 1;

    const chunkData = chunks.find(
      (chunk) =>
        chunk.startLine === chunkStartLine && chunk.endLine === chunkEndLine
    );

    if (!chunkData) {
      return;
    }

    // How far are we into this chunk in the preview (as a proportion)?
    const previewChunkRect = topChunk.getBoundingClientRect();
    const previewChunkTop =
      previewChunkRect.top -
      viewArea.getBoundingClientRect().top +
      viewScrollTop;
    const pixelsIntoChunk = Math.max(0, viewScrollTop - previewChunkTop);
    const previewChunkHeight = previewChunkRect.height;
    const proportionIntoChunk = pixelsIntoChunk / previewChunkHeight;

    // Apply the same proportion to the visual lines in the editor
    const editorChunkVisualHeight =
      chunkData.visualEndLine - chunkData.visualStartLine + 1;
    const visualLinesIntoChunk = proportionIntoChunk * editorChunkVisualHeight;
    const targetVisualLine = chunkData.visualStartLine + visualLinesIntoChunk;

    // Convert visual line to scroll position
    const lineHeight = parseFloat(getComputedStyle(editArea).lineHeight) || 20;
    const calculatedScrollTop = targetVisualLine * lineHeight;

    // Add 20px safety margin to stay slightly higher in editor (equivalent to ~1 line)
    const targetScrollTop = Math.max(0, calculatedScrollTop - 20);

    // Prevent reverse sync
    viewScrollingRef.current = true;
    editArea.scrollTop = targetScrollTop;

    if (viewScrollingTimerRef.current) {
      clearTimeout(viewScrollingTimerRef.current);
    }
    viewScrollingTimerRef.current = setTimeout(() => {
      viewScrollingRef.current = false;
    }, 50);
  }, [getCurrentViewChunk, chunks]);

  // Throttled scroll handlers
  const editScrollTimerRef = useRef<NodeJS.Timeout | null>(null);
  const viewScrollTimerRef = useRef<NodeJS.Timeout | null>(null);

  const throttledSyncToView = useCallback(() => {
    if (editScrollTimerRef.current) {
      clearTimeout(editScrollTimerRef.current);
    }
    editScrollTimerRef.current = setTimeout(
      syncScrollToView,
      editScrollThrottle
    );
  }, [syncScrollToView, editScrollThrottle]);

  const throttledSyncToEdit = useCallback(() => {
    if (viewScrollTimerRef.current) {
      clearTimeout(viewScrollTimerRef.current);
    }
    viewScrollTimerRef.current = setTimeout(
      syncScrollToEdit,
      viewScrollThrottle
    );
  }, [syncScrollToEdit, viewScrollThrottle]);

  // Set up scroll event listeners
  useEffect(() => {
    const editArea = editAreaRef.current;
    const viewArea = viewAreaRef.current;

    if (!editArea || !viewArea) return;

    editArea.addEventListener("scroll", throttledSyncToView);
    viewArea.addEventListener("scroll", throttledSyncToEdit);

    return () => {
      editArea.removeEventListener("scroll", throttledSyncToView);
      viewArea.removeEventListener("scroll", throttledSyncToEdit);
    };
  }, [throttledSyncToView, throttledSyncToEdit]);

  // Cleanup timers on unmount
  useEffect(() => {
    return () => {
      if (viewScrollingTimerRef.current) {
        clearTimeout(viewScrollingTimerRef.current);
      }
      if (editScrollingTimerRef.current) {
        clearTimeout(editScrollingTimerRef.current);
      }
      if (editScrollTimerRef.current) {
        clearTimeout(editScrollTimerRef.current);
      }
      if (viewScrollTimerRef.current) {
        clearTimeout(viewScrollTimerRef.current);
      }
    };
  }, []);

  return {
    editAreaRef,
    viewAreaRef
  };
}
