// Hook for managing markdown worker and async rendering
import { useCallback, useEffect, useRef, useState } from "react";

// Import types from worker
import type {
  AffectedRegion,
  BlockMapping,
  MarkdownChangeEvent,
  MarkdownErrorResponse,
  MarkdownIncrementalResponse,
  MarkdownInitEvent,
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

  // Initialize worker
  useEffect(() => {
    // Create worker
    const worker = new Worker(
      new URL("../../workers/markdown.worker.ts", import.meta.url),
      { type: "module" }
    );

    workerRef.current = worker;

    // Handle messages from worker
    worker.addEventListener(
      "message",
      (event: MessageEvent<MarkdownWorkerResponse>) => {
        const { type } = event.data;

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

          setHtml(renderedHtml);
          setBlockMappings(renderedBlockMappings);
          setAffectedRegions(renderedAffectedRegions);
          setError(null);
          setIsRendering(false);
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

    // Send an init message to test worker readiness
    const initMessage: MarkdownInitEvent = {
      type: "init-message"
    };

    worker.postMessage(initMessage);

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

      // Set rendering state
      setIsRendering(true);
      setError(null);

      // Send change event to worker
      const message: MarkdownChangeEvent = {
        type: "change-event",
        change,
        fullText
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
