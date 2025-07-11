// Editor State Management and Utilities
//
// This module provides state management utilities for the POD editor
// including default state, validation helpers, and content management.

import { getDefaultEditorContent } from "./monaco";
import { DiagnosticSeverity } from "./types";
import type {
  EditorState,
  EditorActions,
  Diagnostic,
  ExecuteCodeResponse
} from "./types";

/**
 * Create initial editor state
 */
export function createInitialEditorState(): EditorState {
  return {
    fileContent: getDefaultEditorContent(),
    diagnostics: [],
    executionResult: null,
    executionError: null,
    isExecuting: false,
    isValidating: false
  };
}

/**
 * Check if editor has validation errors
 */
export function hasValidationErrors(diagnostics: Diagnostic[]): boolean {
  return diagnostics.some(diagnostic => diagnostic.severity === DiagnosticSeverity.Error);
}

/**
 * Check if editor content is empty or only whitespace/comments
 */
export function isEditorContentEmpty(content: string): boolean {
  // Remove comments and whitespace
  const cleanContent = content
    .replace(/\/\/.*$/gm, '') // Remove line comments
    .replace(/\/\*[\s\S]*?\*\//g, '') // Remove block comments
    .trim();
  
  return cleanContent.length === 0;
}

/**
 * Get first error message from diagnostics
 */
export function getFirstErrorMessage(diagnostics: Diagnostic[]): string | null {
  const firstError = diagnostics.find(d => d.severity === DiagnosticSeverity.Error);
  return firstError ? firstError.message : null;
}

/**
 * Format execution result for display
 */
export function formatExecutionResult(result: ExecuteCodeResponse): string {
  try {
    return JSON.stringify(result, null, 2);
  } catch (error) {
    return `Error formatting result: ${error}`;
  }
}

/**
 * Create editor actions for store integration
 */
export function createEditorActions(
  setState: (partial: Partial<EditorState>) => void,
  getState: () => EditorState,
  validateCode: (code: string) => Promise<Diagnostic[]>,
  executeCode: (code: string, mock: boolean) => Promise<ExecuteCodeResponse>
): EditorActions {
  return {
    setEditorContent: (content: string) => {
      setState({ fileContent: content });
    },

    setEditorDiagnostics: (diagnostics: Diagnostic[]) => {
      setState({ diagnostics });
    },

    setExecutionResult: (result: ExecuteCodeResponse | null) => {
      setState({ 
        executionResult: result,
        executionError: null // Clear error when setting result
      });
    },

    setExecutionError: (error: string | null) => {
      setState({ 
        executionError: error,
        executionResult: null // Clear result when setting error
      });
    },

    setIsExecuting: (executing: boolean) => {
      setState({ isExecuting: executing });
    },

    setIsValidating: (validating: boolean) => {
      setState({ isValidating: validating });
    },

    validateEditorCode: async () => {
      const { fileContent } = getState();
      
      if (isEditorContentEmpty(fileContent)) {
        setState({ diagnostics: [] });
        return;
      }

      setState({ isValidating: true });
      try {
        const diagnostics = await validateCode(fileContent);
        setState({ diagnostics, isValidating: false });
      } catch (error) {
        console.error("Validation failed:", error);
        setState({ 
          diagnostics: [{
            message: error instanceof Error ? error.message : "Validation failed",
            severity: DiagnosticSeverity.Error,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1
          }],
          isValidating: false
        });
      }
    },

    executeEditorCode: async (mock = false) => {
      const { fileContent, diagnostics } = getState();
      
      // Check for validation errors
      if (hasValidationErrors(diagnostics)) {
        const errorMessage = getFirstErrorMessage(diagnostics) || "Code has validation errors";
        setState({ executionError: `Cannot execute: ${errorMessage}` });
        return;
      }

      if (isEditorContentEmpty(fileContent)) {
        setState({ executionError: "Cannot execute empty code" });
        return;
      }

      setState({ 
        isExecuting: true, 
        executionError: null, 
        executionResult: null 
      });

      try {
        const result = await executeCode(fileContent, mock);
        setState({ 
          executionResult: result, 
          isExecuting: false 
        });
      } catch (error) {
        console.error("Execution failed:", error);
        setState({ 
          executionError: error instanceof Error ? error.message : "Execution failed",
          isExecuting: false 
        });
      }
    },

    clearExecutionResults: () => {
      setState({ 
        executionResult: null, 
        executionError: null 
      });
    }
  };
}

/**
 * Debounce utility for validation
 */
export function createDebouncedValidator(
  validateFn: () => Promise<void>,
  delay: number = 300
): () => void {
  let timeoutId: number | null = null;
  
  return () => {
    if (timeoutId !== null) {
      clearTimeout(timeoutId);
    }
    
    timeoutId = window.setTimeout(() => {
      validateFn();
      timeoutId = null;
    }, delay);
  };
}

/**
 * Storage keys for persisting editor state
 */
export const EDITOR_STORAGE_KEYS = {
  FILE_CONTENT: "podlog_editor_content",
  LAST_EXECUTION_MOCK: "podlog_editor_last_mock"
} as const;

/**
 * Save editor content to localStorage
 */
export function saveEditorContent(content: string): void {
  try {
    localStorage.setItem(EDITOR_STORAGE_KEYS.FILE_CONTENT, content);
  } catch (error) {
    console.warn("Failed to save editor content:", error);
  }
}

/**
 * Load editor content from localStorage
 */
export function loadEditorContent(): string {
  try {
    const saved = localStorage.getItem(EDITOR_STORAGE_KEYS.FILE_CONTENT);
    return saved || getDefaultEditorContent();
  } catch (error) {
    console.warn("Failed to load editor content:", error);
    return getDefaultEditorContent();
  }
}

/**
 * Save last used mock setting
 */
export function saveLastMockSetting(mock: boolean): void {
  try {
    localStorage.setItem(EDITOR_STORAGE_KEYS.LAST_EXECUTION_MOCK, String(mock));
  } catch (error) {
    console.warn("Failed to save mock setting:", error);
  }
}

/**
 * Load last used mock setting
 */
export function loadLastMockSetting(): boolean {
  try {
    const saved = localStorage.getItem(EDITOR_STORAGE_KEYS.LAST_EXECUTION_MOCK);
    return saved === "true";
  } catch (error) {
    console.warn("Failed to load mock setting:", error);
    return false;
  }
}