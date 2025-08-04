import Editor, {
  type Monaco,
  type OnChange,
  loader
} from "@monaco-editor/react";
import * as monaco from "monaco-editor/esm/vs/editor/editor.api";
import { useCallback, useEffect, useRef } from "react";
import { useTheme } from "../theme-provider";

import {
  setupMonacoEditor,
  updateEditorMarkers
} from "../../lib/features/authoring/monaco";
import { createDebouncedValidator } from "../../lib/features/authoring/editor";
import { usePodEditor } from "../../lib/store";

// Configure Monaco loader
loader.config({ monaco });

const VALIDATION_DEBOUNCE_MS = 300;

interface EditorPaneProps {
  className?: string;
}

export function EditorPane({ className }: EditorPaneProps) {
  const { theme } = useTheme();

  // Editor state from store
  const {
    editorContent,
    setEditorContent,
    editorDiagnostics,
    validateEditorCode
  } = usePodEditor();

  // Editor refs
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<Monaco | null>(null);

  // Create debounced validator
  const debouncedValidate = useCallback(
    createDebouncedValidator(validateEditorCode, VALIDATION_DEBOUNCE_MS),
    [validateEditorCode]
  );

  // Handle editor content changes
  const handleEditorChange: OnChange = useCallback(
    (value) => {
      const newContent = value || "";
      setEditorContent(newContent);
      debouncedValidate();
    },
    [setEditorContent, debouncedValidate]
  );

  // Update markers when diagnostics change
  useEffect(() => {
    if (editorRef.current && monacoRef.current) {
      updateEditorMarkers(
        editorRef.current,
        monacoRef.current,
        editorDiagnostics
      );
    }
  }, [editorDiagnostics]);

  // Update theme when it changes
  useEffect(() => {
    if (monacoRef.current) {
      const currentTheme = theme === "dark" ? "vs-dark" : "vs-light";
      monacoRef.current.editor.setTheme(currentTheme);
    }
  }, [theme]);

  // Handle editor mount
  const handleEditorDidMount = useCallback(
    (
      mountedEditor: monaco.editor.IStandaloneCodeEditor,
      mountedMonaco: Monaco
    ) => {
      editorRef.current = mountedEditor;
      monacoRef.current = mountedMonaco;

      // Setup Podlang language support
      setupMonacoEditor(mountedEditor, mountedMonaco);

      // Force theme update after language setup
      const currentTheme = theme === "dark" ? "vs-dark" : "vs-light";
      mountedMonaco.editor.setTheme(currentTheme);

      // Initial validation
      if (editorContent.trim()) {
        validateEditorCode();
      }
    },
    [editorContent, validateEditorCode, theme]
  );

  // Determine Monaco theme based on resolved theme
  const editorTheme = theme === "dark" ? "vs-dark" : "vs-light";

  // Debug theme
  console.log("Current theme:", theme, "Monaco theme:", editorTheme);

  return (
    <div
      className={`h-full w-full bg-gray-100 dark:bg-[#1e1e1e] px-1 py-2 ${className || ""}`}
    >
      <Editor
        height="100%"
        width="100%"
        language="Podlang"
        theme={editorTheme}
        value={editorContent}
        onChange={handleEditorChange}
        onMount={handleEditorDidMount}
        options={{
          minimap: { enabled: false },
          fontSize: 14,
          wordWrap: "on",
          scrollBeyondLastLine: false,
          automaticLayout: true,
          lineNumbers: "on",
          renderLineHighlight: "line",
          selectionHighlight: false,
          smoothScrolling: true,
          cursorBlinking: "smooth",
          folding: true,
          foldingHighlight: true,
          bracketPairColorization: {
            enabled: true
          },
          guides: {
            bracketPairs: true,
            indentation: true
          },
          suggest: {
            showKeywords: true,
            showSnippets: true
          }
        }}
      />
    </div>
  );
}
