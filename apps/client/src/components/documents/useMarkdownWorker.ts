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

// Markdown worker hook for async rendering

interface UseMarkdownWorkerResult {
  renderMarkdown: (markdown: string) => void;
  sendChangeEvent: (change: MonacoChange, fullText: string) => void;
  html: string;
  blockMappings: BlockMapping[];
  affectedRegions: AffectedRegion[];
  isRendering: boolean;
  error: string | null;
  isWorkerReady: boolean;
}

export function useMarkdownWorker(): UseMarkdownWorkerResult {
  // State
  const [html, setHtml] = useState<string>("");
  const [blockMappings, setBlockMappings] = useState<BlockMapping[]>([]);
  const [affectedRegions, setAffectedRegions] = useState<AffectedRegion[]>([]);
  const [isRendering, setIsRendering] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isWorkerReady, setIsWorkerReady] = useState(false);

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
        // Create buffer with 1 Int32 slot for sequence coordination
        const buffer = new SharedArrayBuffer(4); // 1 * 4 bytes
        const sharedArray = new Int32Array(buffer);

        // Initialize to 0
        Atomics.store(sharedArray, 0, 0); // latestSequenceId

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

        // Mark worker as ready when we receive any message
        if (!isWorkerReady) {
          setIsWorkerReady(true);
        }

        if (type === "incremental-complete") {
          const {
            html: renderedHtml,
            blockMappings: renderedBlockMappings,
            affectedRegions: renderedAffectedRegions
          } = event.data as MarkdownIncrementalResponse;

          // Always accept the latest sequence
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
      setIsWorkerReady(false); // Mark worker as not ready on error
    });

    // Test worker readiness by sending a ping
    // We'll send an empty change to trigger worker initialization
    const testChange: MonacoChange = {
      range: {
        startLineNumber: 1,
        startColumn: 1,
        endLineNumber: 1,
        endColumn: 1
      },
      rangeLength: 0,
      text: ""
    };

    // Send a test message to initialize the worker
    const testMessage = {
      type: "change-event" as const,
      change: testChange,
      fullText: "",
      sequenceId: 0,
      sharedBuffer: sharedBufferRef.current || undefined
    };

    worker.postMessage(testMessage);

    // Cleanup
    return () => {
      worker.terminate();
      workerRef.current = null;
      setIsWorkerReady(false);
    };
  }, []);

  // Change event function for optimized rendering
  const sendChangeEvent = useCallback(
    (change: MonacoChange, fullText: string) => {
      const worker = workerRef.current;
      if (!worker) {
        console.log("⚠️ sendChangeEvent called but worker not ready");
        return;
      }

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
      // For full rendering, simulate a change event covering the actual document
      const lineCount = markdown.split("\n").length;
      const fakeChange: MonacoChange = {
        range: {
          startLineNumber: 1,
          startColumn: 1,
          endLineNumber: lineCount,
          endColumn: 1
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
    error,
    isWorkerReady
  };
}
