// Monaco Editor Web Workers Configuration for Vite
// This configures MonacoEnvironment to use Vite's worker system

import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";

// Configure Monaco Environment with proper worker handling
export function initializeMonacoWorkers() {
  // Set up MonacoEnvironment globally (use any to avoid type conflicts)
  (window as any).MonacoEnvironment = {
    getWorker(_: string, _label: string): Worker {
      // We only use markdown and Podlang languages in this app
      // All languages fall back to the base editor worker
      return new editorWorker();
    }
  };
}

// For server-side rendering compatibility
export function isWorkerSupported(): boolean {
  return typeof Worker !== "undefined" && typeof window !== "undefined";
}
