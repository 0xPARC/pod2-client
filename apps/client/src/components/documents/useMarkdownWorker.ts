// Hook for managing markdown worker and async rendering
import { useCallback, useEffect, useRef, useState } from "react";

// Import types from worker
import type {
  AffectedRegion,
  BlockMapping,
  MarkdownChangeEvent,
  MarkdownErrorResponse,
  MarkdownIncrementalResponse,
  MarkdownWorkerResponse,
  MonacoChange
} from "../../workers/markdown.worker";

// No options needed - always use optimized rendering

interface UseMarkdownWorkerResult {
  renderMarkdown: (markdown: string) => void;
  sendChangeEvent: (change: MonacoChange, fullText: string) => void;
  html: string;
  blockMappings: BlockMapping[];
  affectedRegions: AffectedRegion[];
  isRendering: boolean;
  error: string | null;
  // Optimized rendering with change tracking
}

export function useMarkdownWorker(): UseMarkdownWorkerResult {
  // Uses optimized rendering with SharedArrayBuffer coordination

  // State
  const [html, setHtml] = useState<string>("");
  const [blockMappings, setBlockMappings] = useState<BlockMapping[]>([]);
  const [affectedRegions, setAffectedRegions] = useState<AffectedRegion[]>([]);
  const [isRendering, setIsRendering] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Refs for worker management
  const workerRef = useRef<Worker | null>(null);
  const changeSequenceIdRef = useRef(0);
  const sharedBufferRef = useRef<SharedArrayBuffer | null>(null);

  // Initialize worker and shared buffer
  useEffect(() => {
    // Create worker
    const worker = new Worker(
      new URL("../../workers/markdown.worker.ts", import.meta.url),
      { type: "module" }
    );

    workerRef.current = worker;

    // Create SharedArrayBuffer for coordination (if supported)
    if (typeof SharedArrayBuffer !== "undefined") {
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

        if (type === "incremental-complete") {
          const {
            html: renderedHtml,
            blockMappings: renderedBlockMappings,
            affectedRegions: renderedAffectedRegions
          } = event.data as MarkdownIncrementalResponse;

          // For incremental mode, always accept the latest sequence
          if (sequenceId === changeSequenceIdRef.current) {
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
  }, []);

  // Change event function for optimized rendering
  const sendChangeEvent = useCallback(
    (change: MonacoChange, fullText: string) => {
      const worker = workerRef.current;
      if (!worker) return;

      // Increment change sequence ID
      const sequenceId = ++changeSequenceIdRef.current;

      // Update shared buffer with latest sequence ID
      if (sharedBufferRef.current) {
        const sharedArray = new Int32Array(sharedBufferRef.current);
        Atomics.store(sharedArray, 0, sequenceId);
      }

      // Set rendering state
      setIsRendering(true);
      setError(null);

      // Send change event to worker
      const message: MarkdownChangeEvent = {
        type: "change-event",
        change,
        fullText,
        sequenceId,
        sharedBuffer: sharedBufferRef.current || undefined
      };

      worker.postMessage(message);
    },
    []
  );

  // Render function using change event simulation
  const renderMarkdown = useCallback(
    (markdown: string) => {
      // For full rendering, simulate a change event covering the entire document
      const fakeChange: MonacoChange = {
        range: {
          startLineNumber: 1,
          startColumn: 1,
          endLineNumber: Number.MAX_SAFE_INTEGER,
          endColumn: Number.MAX_SAFE_INTEGER
        },
        rangeLength: 0,
        text: markdown
      };

      sendChangeEvent(fakeChange, markdown);
    },
    [sendChangeEvent]
  );

  return {
    renderMarkdown,
    sendChangeEvent,
    html,
    blockMappings,
    affectedRegions,
    isRendering,
    error
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
