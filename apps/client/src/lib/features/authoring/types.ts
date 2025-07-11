// Editor Types and Interfaces
//
// This module defines types for the POD editor functionality including
// validation diagnostics, execution results, and editor state.

/**
 * Diagnostic severity levels for code validation
 */
export enum DiagnosticSeverity {
  Error = "Error",
  Warning = "Warning", 
  Information = "Information",
  Hint = "Hint"
}

/**
 * A diagnostic message from code validation
 */
export interface Diagnostic {
  message: string;
  severity: DiagnosticSeverity;
  start_line: number; // 1-indexed
  start_column: number; // 1-indexed
  end_line: number; // 1-indexed
  end_column: number; // 1-indexed
}

/**
 * Request payload for code validation
 */
export interface ValidateCodeRequest {
  code: string;
}

/**
 * Response from code validation
 */
export interface ValidateCodeResponse {
  diagnostics: Diagnostic[];
}

/**
 * Request payload for code execution
 */
export interface ExecuteCodeRequest {
  code: string;
  mock: boolean;
}

/**
 * Response from code execution
 */
export interface ExecuteCodeResponse {
  main_pod: any; // MainPod structure from POD2
  diagram: string; // Mermaid diagram markdown
}

/**
 * Error response from backend operations
 */
export interface EditorError {
  message: string;
  code?: string;
  details?: any;
}

/**
 * Editor state for the application store
 */
export interface EditorState {
  fileContent: string;
  diagnostics: Diagnostic[];
  executionResult: ExecuteCodeResponse | null;
  executionError: string | null;
  isExecuting: boolean;
  isValidating: boolean;
}

/**
 * Actions for editor state management
 */
export interface EditorActions {
  setEditorContent: (content: string) => void;
  setEditorDiagnostics: (diagnostics: Diagnostic[]) => void;
  setExecutionResult: (result: ExecuteCodeResponse | null) => void;
  setExecutionError: (error: string | null) => void;
  setIsExecuting: (executing: boolean) => void;
  setIsValidating: (validating: boolean) => void;
  validateEditorCode: () => Promise<void>;
  executeEditorCode: (mock?: boolean) => Promise<void>;
  clearExecutionResults: () => void;
}

/**
 * Combined editor store interface
 */
export interface EditorStore extends EditorState, EditorActions {}