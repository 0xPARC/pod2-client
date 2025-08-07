import Editor, {
  type Monaco,
  type OnChange,
  loader
} from "@monaco-editor/react";
import "highlight.js/styles/github-dark.css";
import {
  BoldIcon,
  CodeIcon,
  EditIcon,
  EyeIcon,
  ItalicIcon,
  LinkIcon,
  ListIcon,
  QuoteIcon,
  SplitIcon
} from "lucide-react";
import * as monaco from "monaco-editor/esm/vs/editor/editor.api";
import React, { useCallback, useEffect, useRef, useState } from "react";
import {
  initializeMonacoWorkers,
  isWorkerSupported
} from "../../../lib/monacoWorkers";
import { useTheme } from "../../theme-provider";
import { Button } from "../../ui/button";
import { IncrementalMarkdownPreview } from "../IncrementalMarkdownPreview";
import { useMarkdownWorker } from "../useMarkdownWorker";
import { useScrollSync } from "../useScrollSync";

// Configure Monaco loader and workers
loader.config({ monaco });

// Initialize Monaco workers if supported
if (isWorkerSupported()) {
  initializeMonacoWorkers();
}

// WeakMap to track disposables for editor cleanup
const editorDisposables = new WeakMap<
  monaco.editor.IStandaloneCodeEditor,
  monaco.IDisposable
>();

// Constants for scroll sync configuration
const SCROLL_SYNC_COOLDOWN_MS = 150; // Prevents feedback loops between editor/preview scrolling

interface MarkdownEditorProps {
  value: string;
  onChange: (value: string) => void;
  className?: string;
}

type ViewMode = "edit" | "preview" | "split";

export function MarkdownEditor({
  value,
  onChange,
  className
}: MarkdownEditorProps) {
  const [viewMode, setViewMode] = useState<ViewMode>("split");
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<Monaco | null>(null);
  const previewContainerRef = useRef<HTMLDivElement | null>(null);
  const { theme } = useTheme();

  // Use worker-based markdown renderer with incremental updates
  const {
    renderMarkdown,
    sendChangeEvent,
    html: renderedHtml,
    blockMappings,
    affectedRegions,
    error,
    isIncrementalMode
  } = useMarkdownWorker({
    enableIncremental: true // Re-enable incremental rendering with debugging
  });

  // Use scroll synchronization with cooldown to prevent feedback loops
  const { setEditorRef, setPreviewRef, updateBlockMappings } = useScrollSync({
    cooldownMs: SCROLL_SYNC_COOLDOWN_MS
  });

  // Handle preview container ref
  const handlePreviewRef = useCallback(
    (element: HTMLDivElement | null) => {
      previewContainerRef.current = element;
      setPreviewRef(element);
    },
    [setPreviewRef]
  );

  // Initial rendering when component mounts
  useEffect(() => {
    if (value.trim()) {
      renderMarkdown(value);
    }
  }, [renderMarkdown]); // Only run on mount, not on value changes

  // Update block mappings when they change
  useEffect(() => {
    if (blockMappings.length > 0) {
      updateBlockMappings(blockMappings);
    }
  }, [blockMappings, updateBlockMappings]);

  // Display content based on state
  const displayHtml = error
    ? `<div style="color: red; padding: 16px;">Error rendering markdown: ${error}</div>`
    : value.trim()
      ? renderedHtml ||
        '<div style="padding: 16px; opacity: 0.5;">Rendering...</div>'
      : "Nothing to preview yet. Start typing to see your markdown rendered here.";

  // Handle Monaco editor content changes
  const handleEditorChange: OnChange = useCallback(
    (value) => {
      const newContent = value || "";
      onChange(newContent);

      // In incremental mode, changes are handled by onDidChangeContent
      // In legacy mode, trigger full rendering
      if (!isIncrementalMode) {
        renderMarkdown(newContent);
      }
    },
    [onChange, isIncrementalMode, renderMarkdown]
  );

  // Handle Monaco editor mount
  const handleEditorDidMount = useCallback(
    (
      mountedEditor: monaco.editor.IStandaloneCodeEditor,
      mountedMonaco: Monaco
    ) => {
      editorRef.current = mountedEditor;
      monacoRef.current = mountedMonaco;

      // Set theme
      const currentTheme = theme === "dark" ? "vs-dark" : "vs-light";
      mountedMonaco.editor.setTheme(currentTheme);

      // Set editor ref for scroll sync
      setEditorRef(mountedEditor);

      // Set up incremental change handling
      if (isIncrementalMode) {
        const model = mountedEditor.getModel();
        if (model) {
          const disposable = model.onDidChangeContent((event) => {
            const changes = event.changes;
            if (changes.length > 0) {
              const change = changes[0];
              const fullText = model.getValue();
              sendChangeEvent(change, fullText);
            }
          });

          // Store disposable for cleanup using WeakMap
          editorDisposables.set(mountedEditor, disposable);
        }
      }
    },
    [theme, setEditorRef, isIncrementalMode, sendChangeEvent]
  );

  // Update theme when it changes
  React.useEffect(() => {
    if (monacoRef.current) {
      const currentTheme = theme === "dark" ? "vs-dark" : "vs-light";
      monacoRef.current.editor.setTheme(currentTheme);
    }
  }, [theme]);

  // Cleanup incremental event listeners on unmount
  React.useEffect(() => {
    return () => {
      const editor = editorRef.current;
      if (editor) {
        const disposable = editorDisposables.get(editor);
        if (disposable) {
          disposable.dispose();
          editorDisposables.delete(editor);
        }
      }
    };
  }, []);

  // Insert markdown formatting at cursor position using Monaco API
  const insertFormatting = useCallback(
    (prefix: string, suffix: string = "", placeholder: string = "") => {
      const editor = editorRef.current;
      if (!editor) return;

      const selection = editor.getSelection();
      if (!selection) return;

      const model = editor.getModel();
      if (!model) return;

      const selectedText = model.getValueInRange(selection);
      const replacement = selectedText
        ? `${prefix}${selectedText}${suffix}`
        : `${prefix}${placeholder}${suffix}`;

      // Execute edit operation
      editor.executeEdits("markdown-formatting", [
        {
          range: selection,
          text: replacement
        }
      ]);

      // Set selection after the inserted text
      if (selectedText) {
        // Select the replaced text
        editor.setSelection({
          startLineNumber: selection.startLineNumber,
          startColumn: selection.startColumn + prefix.length,
          endLineNumber: selection.endLineNumber,
          endColumn: selection.startColumn + prefix.length + selectedText.length
        });
      } else {
        // Select the placeholder text
        editor.setSelection({
          startLineNumber: selection.startLineNumber,
          startColumn: selection.startColumn + prefix.length,
          endLineNumber: selection.startLineNumber,
          endColumn: selection.startColumn + prefix.length + placeholder.length
        });
      }

      editor.focus();
    },
    []
  );

  // Toolbar actions
  const handleBold = () => insertFormatting("**", "**", "bold text");
  const handleItalic = () => insertFormatting("*", "*", "italic text");
  const handleLink = () => insertFormatting("[", "](url)", "link text");
  const handleList = () => insertFormatting("- ", "", "list item");
  const handleQuote = () => insertFormatting("> ", "", "quote");
  const handleCode = () => insertFormatting("`", "`", "code");

  return (
    <div className={`flex flex-col ${className}`}>
      {/* Toolbar */}
      <div className="flex items-center justify-between p-2 border-b bg-muted">
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            onClick={handleBold}
            title="Bold (Ctrl+B)"
          >
            <BoldIcon className="w-4 h-4" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={handleItalic}
            title="Italic (Ctrl+I)"
          >
            <ItalicIcon className="w-4 h-4" />
          </Button>
          <Button variant="ghost" size="sm" onClick={handleLink} title="Link">
            <LinkIcon className="w-4 h-4" />
          </Button>
          <Button variant="ghost" size="sm" onClick={handleList} title="List">
            <ListIcon className="w-4 h-4" />
          </Button>
          <Button variant="ghost" size="sm" onClick={handleQuote} title="Quote">
            <QuoteIcon className="w-4 h-4" />
          </Button>
          <Button variant="ghost" size="sm" onClick={handleCode} title="Code">
            <CodeIcon className="w-4 h-4" />
          </Button>
        </div>

        {/* View mode toggle */}
        <div className="flex items-center gap-1">
          <Button
            variant={viewMode === "edit" ? "default" : "ghost"}
            size="sm"
            onClick={() => setViewMode("edit")}
            title="Edit Only"
          >
            <EditIcon className="w-4 h-4" />
          </Button>
          <Button
            variant={viewMode === "split" ? "default" : "ghost"}
            size="sm"
            onClick={() => setViewMode("split")}
            title="Split View"
          >
            <SplitIcon className="w-4 h-4" />
          </Button>
          <Button
            variant={viewMode === "preview" ? "default" : "ghost"}
            size="sm"
            onClick={() => setViewMode("preview")}
            title="Preview Only"
          >
            <EyeIcon className="w-4 h-4" />
          </Button>
        </div>
      </div>

      {/* Editor/Preview Content */}
      <div className="flex flex-1 min-h-0">
        {/* Editor pane */}
        {(viewMode === "edit" || viewMode === "split") && (
          <div
            className={`${viewMode === "split" ? "w-1/2" : "w-full"} flex flex-col min-h-0`}
          >
            <div className="flex-1 min-h-0">
              <Editor
                height="100%"
                width="100%"
                language="markdown"
                theme={theme === "dark" ? "vs-dark" : "vs-light"}
                value={value}
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
                  // Disable IntelliSense/autocomplete features inappropriate for markdown
                  quickSuggestions: false,
                  suggestOnTriggerCharacters: false,
                  acceptSuggestionOnEnter: "off",
                  tabCompletion: "off",
                  wordBasedSuggestions: "off",
                  // Disable parameter hints and signature help
                  parameterHints: { enabled: false },
                  // Disable code lens and other code-oriented features
                  codeLens: false,
                  // Disable hover information
                  hover: { enabled: false },
                  // Keep basic bracket features but disable advanced code features
                  bracketPairColorization: {
                    enabled: false // Disable for markdown
                  },
                  guides: {
                    bracketPairs: false, // Not useful for markdown
                    indentation: false // Markdown doesn't need indentation guides
                  },
                  // Disable suggestions entirely
                  suggest: {
                    showKeywords: false,
                    showSnippets: false,
                    showFunctions: false,
                    showConstructors: false,
                    showFields: false,
                    showVariables: false,
                    showClasses: false,
                    showStructs: false,
                    showInterfaces: false,
                    showModules: false,
                    showProperties: false,
                    showEvents: false,
                    showOperators: false,
                    showUnits: false,
                    showValues: false,
                    showConstants: false,
                    showEnums: false,
                    showEnumMembers: false,
                    showColors: false,
                    showFiles: false,
                    showReferences: false,
                    showFolders: false,
                    showTypeParameters: false,
                    showIssues: false,
                    showUsers: false,
                    showWords: false
                  },
                  padding: {
                    top: 16,
                    bottom: 16
                  },
                  tabSize: 2,
                  insertSpaces: true
                }}
              />
            </div>
          </div>
        )}

        {/* Preview pane */}
        {(viewMode === "preview" || viewMode === "split") && (
          <div
            className={`${viewMode === "split" ? "w-1/2 border-l" : "w-full"} flex flex-col min-h-0 min-w-0 bg-card`}
          >
            <IncrementalMarkdownPreview
              ref={handlePreviewRef}
              html={displayHtml}
              affectedRegions={affectedRegions}
              blockMappings={blockMappings}
              isIncrementalMode={isIncrementalMode}
              className="flex-1 min-h-0 min-w-0 p-4 overflow-auto prose prose-neutral max-w-none dark:prose-invert prose-headings:font-semibold prose-h1:text-2xl prose-h2:text-xl prose-h3:text-lg prose-pre:bg-muted prose-pre:border prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:before:content-none prose-code:after:content-none prose-pre:overflow-x-auto prose-code:break-all [&_table]:overflow-x-auto [&_table]:max-w-full [&_*]:max-w-full [&_*]:overflow-wrap-anywhere"
            />
          </div>
        )}
      </div>
    </div>
  );
}
