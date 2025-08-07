// Monaco Editor Web Workers Configuration for Vite
// This configures MonacoEnvironment to use Vite's worker system

import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import jsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import cssWorker from "monaco-editor/esm/vs/language/css/css.worker?worker";
import htmlWorker from "monaco-editor/esm/vs/language/html/html.worker?worker";
import tsWorker from "monaco-editor/esm/vs/language/typescript/ts.worker?worker";

// Configure Monaco Environment with proper worker handling
export function initializeMonacoWorkers() {
  // Set up MonacoEnvironment globally (use any to avoid type conflicts)
  (window as any).MonacoEnvironment = {
    getWorker(_: string, label: string): Worker {
      // TypeScript and JavaScript workers
      if (label === "typescript" || label === "javascript") {
        return new tsWorker();
      }

      // CSS and related language workers
      if (label === "css" || label === "scss" || label === "less") {
        return new cssWorker();
      }

      // HTML and template language workers
      if (label === "html" || label === "handlebars" || label === "razor") {
        return new htmlWorker();
      }

      // JSON worker
      if (label === "json") {
        return new jsonWorker();
      }

      // Default editor worker for all other cases
      return new editorWorker();
    }
  };
}

// For server-side rendering compatibility
export function isWorkerSupported(): boolean {
  return typeof Worker !== "undefined" && typeof window !== "undefined";
}
