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
import { useCallback, useRef, useState } from "react";
import { Button } from "../../ui/button";
import { Textarea } from "../../ui/textarea";
import { useChunkBasedScrollSync } from "../useChunkBasedScrollSync";
import { useChunkedMarkdown } from "../useChunkedMarkdown";

interface MarkdownEditorProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  className?: string;
}

type ViewMode = "edit" | "preview" | "split";

export function MarkdownEditor({
  value,
  onChange,
  placeholder,
  className
}: MarkdownEditorProps) {
  const [viewMode, setViewMode] = useState<ViewMode>("split");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Use chunked markdown rendering
  const { chunkElements: chunkedContent, chunks } = useChunkedMarkdown(
    value,
    textareaRef
  );

  // Use chunk-based scroll synchronization
  const { editAreaRef, viewAreaRef } = useChunkBasedScrollSync(chunks);

  // Combined ref callback to handle both textareaRef and editAreaRef
  const setTextareaRef = useCallback((element: HTMLTextAreaElement | null) => {
    textareaRef.current = element;
    editAreaRef.current = element;
  }, []);

  // Insert markdown formatting at cursor position
  const insertFormatting = useCallback(
    (prefix: string, suffix: string = "", placeholder: string = "") => {
      const textarea = textareaRef.current;
      if (!textarea) return;

      const start = textarea.selectionStart;
      const end = textarea.selectionEnd;
      const selectedText = value.substring(start, end);

      const replacement = selectedText
        ? `${prefix}${selectedText}${suffix}`
        : `${prefix}${placeholder}${suffix}`;

      const newValue =
        value.substring(0, start) + replacement + value.substring(end);
      onChange(newValue);

      // Set cursor position after the inserted text
      setTimeout(() => {
        if (selectedText) {
          textarea.setSelectionRange(
            start + prefix.length,
            start + prefix.length + selectedText.length
          );
        } else {
          textarea.setSelectionRange(
            start + prefix.length,
            start + prefix.length + placeholder.length
          );
        }
        textarea.focus();
      }, 0);
    },
    [value, onChange]
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
      <div className="flex flex-1 min-h-0 min-w-0 w-full max-w-full overflow-hidden">
        {/* Editor pane */}
        {(viewMode === "edit" || viewMode === "split") && (
          <div
            className={`${viewMode === "split" ? "w-1/2" : "w-full"} flex flex-col min-h-0 min-w-0`}
          >
            <Textarea
              ref={setTextareaRef}
              value={value}
              onChange={(e) => onChange(e.target.value)}
              placeholder={placeholder || "Enter your markdown content..."}
              className="flex-1 min-h-0 border-0 rounded-none resize-none focus-visible:ring-0 font-mono text-base md:text-base overflow-auto p-4"
            />
          </div>
        )}

        {/* Preview pane */}
        {(viewMode === "preview" || viewMode === "split") && (
          <div
            className={`${viewMode === "split" ? "w-1/2 border-l" : "w-full"} flex flex-col min-h-0 min-w-0 max-w-full bg-card overflow-hidden`}
          >
            <div
              ref={viewAreaRef}
              className="flex-1 min-h-0 min-w-0 max-w-full p-4 overflow-auto prose prose-neutral max-w-none dark:prose-invert prose-headings:font-semibold prose-h1:text-2xl prose-h2:text-xl prose-h3:text-lg prose-pre:bg-muted prose-pre:border prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:before:content-none prose-code:after:content-none prose-pre:overflow-x-auto prose-code:break-all [&_table]:overflow-x-auto [&_table]:max-w-full [&_*]:max-w-full [&_*]:overflow-wrap-anywhere"
              style={{ wordBreak: "break-word", overflowWrap: "break-word" }}
            >
              {chunkedContent}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
