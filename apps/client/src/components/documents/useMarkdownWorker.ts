// Hook for managing markdown worker and async rendering
import { useCallback, useEffect, useRef, useState } from "react";

// Import types from worker
import type {
  MarkdownWorkerResponse,
  MarkdownRenderRequest,
  MarkdownChangeEvent,
  MarkdownRenderResponse,
  MarkdownIncrementalResponse,
  MarkdownErrorResponse,
  BlockMapping,
  AffectedRegion,
  MonacoChange
} from "../../workers/markdown.worker";

interface UseMarkdownWorkerOptions {
  // Enable SharedArrayBuffer coordination (set to false to disable for compatibility)
  useSharedBuffer?: boolean;
  // Enable incremental rendering with change events
  enableIncremental?: boolean;
}

interface UseMarkdownWorkerResult {
  renderMarkdown: (markdown: string) => void;
  sendChangeEvent: (change: MonacoChange, fullText: string) => void;
  html: string;
  blockMappings: BlockMapping[];
  affectedRegions: AffectedRegion[];
  isRendering: boolean;
  error: string | null;
  isIncrementalMode: boolean;
}

export function useMarkdownWorker(
  options: UseMarkdownWorkerOptions = {}
): UseMarkdownWorkerResult {
  const { useSharedBuffer = true, enableIncremental = true } = options;

  // State
  const [html, setHtml] = useState<string>("");
  const [blockMappings, setBlockMappings] = useState<BlockMapping[]>([]);
  const [affectedRegions, setAffectedRegions] = useState<AffectedRegion[]>([]);
  const [isRendering, setIsRendering] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Refs for worker management
  const workerRef = useRef<Worker | null>(null);
  const sequenceIdRef = useRef(0);
  const changeSequenceIdRef = useRef(0);
  const sharedBufferRef = useRef<SharedArrayBuffer | null>(null);
  const pendingRenderRef = useRef<string | null>(null);

  // Initialize worker and shared buffer
  useEffect(() => {
    // Create worker
    const worker = new Worker(
      new URL("../../workers/markdown.worker.ts", import.meta.url),
      { type: "module" }
    );

    workerRef.current = worker;

    // Create SharedArrayBuffer for coordination (if supported and enabled)
    if (useSharedBuffer && typeof SharedArrayBuffer !== "undefined") {
      try {
        // Create buffer with 3 Int32 slots: [latestSequenceId, completedSequenceId, lastRenderedSequenceId]
        const buffer = new SharedArrayBuffer(3 * 4); // 3 * 4 bytes
        const sharedArray = new Int32Array(buffer);

        // Initialize all values to 0
        Atomics.store(sharedArray, 0, 0); // latestSequenceId
        Atomics.store(sharedArray, 1, 0); // completedSequenceId
        Atomics.store(sharedArray, 2, 0); // lastRenderedSequenceId (for diagnostics)

        sharedBufferRef.current = buffer;
      } catch (e) {
        console.warn(
          "SharedArrayBuffer not supported, falling back to basic mode:",
          e
        );
        sharedBufferRef.current = null;
      }
    }

    // Handle messages from worker
    worker.addEventListener(
      "message",
      (event: MessageEvent<MarkdownWorkerResponse>) => {
        const { type, sequenceId } = event.data;

        if (type === "render-complete") {
          const { html: renderedHtml, blockMappings: renderedBlockMappings } =
            event.data as MarkdownRenderResponse;

          // Only update if this is still the latest completed render
          if (sharedBufferRef.current) {
            const sharedArray = new Int32Array(sharedBufferRef.current);
            const completedSequenceId = Atomics.load(sharedArray, 1);

            if (sequenceId === completedSequenceId) {
              setHtml(renderedHtml);
              setBlockMappings(renderedBlockMappings);
              setAffectedRegions([]); // Legacy mode has no affected regions
              setError(null);
              setIsRendering(false);

              // Update diagnostics
              Atomics.store(sharedArray, 2, sequenceId);
            }
          } else {
            // Without SharedArrayBuffer, just check sequence ID
            if (sequenceId >= sequenceIdRef.current - 1) {
              // Allow last or current
              setHtml(renderedHtml);
              setBlockMappings(renderedBlockMappings);
              setAffectedRegions([]);
              setError(null);
              setIsRendering(false);
            }
          }
        } else if (type === "incremental-complete") {
          const {
            html: renderedHtml,
            blockMappings: renderedBlockMappings,
            affectedRegions: renderedAffectedRegions
          } = event.data as MarkdownIncrementalResponse;

          // For incremental mode, always accept the latest sequence
          if (sequenceId >= changeSequenceIdRef.current - 5) {
            // Allow some flexibility
            setHtml(renderedHtml);
            setBlockMappings(renderedBlockMappings);
            setAffectedRegions(renderedAffectedRegions);
            setError(null);
            setIsRendering(false);
          }
        } else if (type === "error") {
          const { error: errorMessage } = event.data as MarkdownErrorResponse;
          setError(errorMessage);
          setIsRendering(false);
        }
      }
    );

    // Handle worker errors
    worker.addEventListener("error", (event) => {
      console.error("Markdown worker error:", event);
      setError("Worker error occurred");
      setIsRendering(false);
    });

    // Cleanup
    return () => {
      worker.terminate();
      workerRef.current = null;
    };
  }, [useSharedBuffer]);

  // Legacy render function (for backward compatibility)
  const renderMarkdown = useCallback((markdown: string) => {
    const worker = workerRef.current;
    if (!worker) return;

    // Increment sequence ID
    const sequenceId = ++sequenceIdRef.current;

    // Update shared buffer with latest sequence ID
    if (sharedBufferRef.current) {
      const sharedArray = new Int32Array(sharedBufferRef.current);
      Atomics.store(sharedArray, 0, sequenceId);
    }

    // Set rendering state
    setIsRendering(true);
    setError(null);
    pendingRenderRef.current = markdown;

    // Send message to worker
    const message: MarkdownRenderRequest = {
      type: "render",
      markdown,
      sequenceId,
      sharedBuffer: sharedBufferRef.current || undefined
    };

    worker.postMessage(message);
  }, []);

  // New change event function for incremental rendering
  const sendChangeEvent = useCallback(
    (change: MonacoChange, fullText: string) => {
      const worker = workerRef.current;
      if (!worker || !enableIncremental) {
        // Fallback to legacy rendering
        renderMarkdown(fullText);
        return;
      }

      // Increment change sequence ID
      const sequenceId = ++changeSequenceIdRef.current;

      // Set rendering state
      setIsRendering(true);
      setError(null);

      // Send change event to worker
      const message: MarkdownChangeEvent = {
        type: "change-event",
        change,
        fullText,
        sequenceId,
        latestSequenceId: sequenceId // This will be updated if more changes are pending
      };

      worker.postMessage(message);
    },
    [enableIncremental, renderMarkdown]
  );

  return {
    renderMarkdown,
    sendChangeEvent,
    html,
    blockMappings,
    affectedRegions,
    isRendering,
    error,
    isIncrementalMode: enableIncremental
  };
}

// Hook for diagnostic information (optional)
export function useMarkdownWorkerDiagnostics(
  _workerResult: UseMarkdownWorkerResult
) {
  const [diagnostics] = useState({
    droppedRenders: 0,
    latestSequenceId: 0,
    completedSequenceId: 0,
    lastRenderedSequenceId: 0
  });

  // This would be updated by the worker hook if diagnostics are enabled
  // For now, this is a placeholder for future diagnostic features

  return diagnostics;
}
