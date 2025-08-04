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
import ReactMarkdown from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import rehypeMathjax from "rehype-mathjax";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import { Button } from "../../ui/button";
import { Textarea } from "../../ui/textarea";
import rehypeDisplayMath from "../displayMath";

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
      <div className="flex flex-1 min-h-0">
        {/* Editor pane */}
        {(viewMode === "edit" || viewMode === "split") && (
          <div
            className={`${viewMode === "split" ? "w-1/2" : "w-full"} flex flex-col min-h-0`}
          >
            <Textarea
              ref={textareaRef}
              value={value}
              onChange={(e) => onChange(e.target.value)}
              placeholder={placeholder || "Enter your markdown content..."}
              className="flex-1 min-h-0 border-0 rounded-none resize-none focus-visible:ring-0 font-mono text-sm overflow-auto p-4"
            />
          </div>
        )}

        {/* Preview pane */}
        {(viewMode === "preview" || viewMode === "split") && (
          <div
            className={`${viewMode === "split" ? "w-1/2 border-l" : "w-full"} flex flex-col min-h-0 min-w-0 bg-card`}
          >
            <div className="flex-1 min-h-0 min-w-0 p-4 overflow-auto prose prose-neutral max-w-none dark:prose-invert prose-headings:font-semibold prose-h1:text-2xl prose-h2:text-xl prose-h3:text-lg prose-pre:bg-muted prose-pre:border prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:text-sm prose-code:before:content-none prose-code:after:content-none prose-pre:overflow-x-auto prose-code:break-all [&_.MathJax]:overflow-x-auto [&_.MathJax]:max-w-full [&_mjx-container]:overflow-x-auto [&_mjx-container]:max-w-full [&_svg]:max-w-full [&_svg]:overflow-visible [&_.MathJax_Error]:bg-destructive/10 [&_.MathJax_Error]:border [&_.MathJax_Error]:border-destructive/20 [&_.MathJax_Error]:rounded [&_.MathJax_Error]:p-2 [&_.MathJax_Error]:min-h-[2rem]">
              <ReactMarkdown
                remarkPlugins={[
                  remarkGfm,
                  [remarkMath, { singleDollarTextMath: true }]
                ]}
                rehypePlugins={[
                  rehypeHighlight,
                  [
                    rehypeMathjax,
                    {
                      // MathJax configuration
                      tex: {
                        inlineMath: [
                          ["$", "$"],
                          ["\\(", "\\)"]
                        ],
                        displayMath: [
                          ["$$", "$$"],
                          ["\\[", "\\]"]
                        ],
                        loader: {
                          load: [
                            "[tex]/textmacros",
                            "[tex]/textcomp",
                            "[tex]/noerrors"
                          ]
                        },
                        tex: {
                          packages: { "[+]": ["textmacros", "noerrors"] }
                        },
                        textmacros: { packages: { "[+]": ["textcomp"] } },
                        processEscapes: true,
                        macros: {
                          "\\RR": "\\mathbb{R}",
                          "\\NN": "\\mathbb{N}"
                        }
                      }
                    }
                  ],
                  rehypeDisplayMath
                ]}
                components={{
                  // Custom link component to handle external links safely
                  a: ({ href, children, ...props }) => (
                    <a
                      href={href}
                      target="_blank"
                      rel="noopener noreferrer"
                      {...props}
                    >
                      {children}
                    </a>
                  ),
                  // Custom table components for better styling
                  table: ({ children, ...props }) => (
                    <div className="overflow-x-auto">
                      <table
                        className="min-w-full divide-y divide-border"
                        {...props}
                      >
                        {children}
                      </table>
                    </div>
                  ),
                  thead: ({ children, ...props }) => (
                    <thead className="bg-muted" {...props}>
                      {children}
                    </thead>
                  ),
                  th: ({ children, ...props }) => (
                    <th
                      className="px-4 py-3 text-left text-xs font-medium text-muted-foreground uppercase tracking-wider"
                      {...props}
                    >
                      {children}
                    </th>
                  ),
                  td: ({ children, ...props }) => (
                    <td
                      className="px-4 py-3 whitespace-nowrap text-sm border-t"
                      {...props}
                    >
                      {children}
                    </td>
                  )
                }}
              >
                {value ||
                  "Nothing to preview yet. Start typing to see your markdown rendered here."}
              </ReactMarkdown>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
