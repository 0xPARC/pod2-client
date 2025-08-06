import React, { useMemo, useRef } from "react";
import { useMarkdownRenderer, renderMarkdownToHtml } from "./markdownRenderer";

interface MarkdownChunk {
  id: string;
  startLine: number; // String line number (0-based)
  endLine: number; // String line number (0-based)
  visualStartLine: number; // First visual line in textarea (0-based)
  visualEndLine: number; // Last visual line in textarea (0-based)
  content: string;
  renderedHtml: string;
  hash: string;
}

// Simple hash function for chunk content
function hashString(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    const char = str.charCodeAt(i);
    hash = (hash << 5) - hash + char;
    hash = hash & hash; // Convert to 32-bit integer
  }
  return hash.toString(36);
}

// Measure how many characters fit per line in a textarea
function measureTextareaCharWidth(textarea: HTMLTextAreaElement): number {
  // Create a temporary element to measure character width
  const measurer = document.createElement("div");
  measurer.style.position = "absolute";
  measurer.style.visibility = "hidden";
  measurer.style.whiteSpace = "pre";
  measurer.style.font = getComputedStyle(textarea).font;
  measurer.textContent = "M".repeat(100); // Use 100 M's for measurement

  document.body.appendChild(measurer);
  const charWidth = measurer.offsetWidth / 100;
  document.body.removeChild(measurer);

  // Calculate usable textarea width (minus padding and scrollbar)
  const computedStyle = getComputedStyle(textarea);
  const paddingLeft = parseFloat(computedStyle.paddingLeft) || 0;
  const paddingRight = parseFloat(computedStyle.paddingRight) || 0;
  const usableWidth = textarea.clientWidth - paddingLeft - paddingRight;

  return Math.floor(usableWidth / charWidth);
}

// Split markdown content into logical blocks with visual line mapping
function splitIntoChunks(
  content: string,
  charsPerLine: number = 80
): Omit<MarkdownChunk, "renderedHtml" | "hash">[] {
  if (!content.trim()) {
    return [];
  }

  const lines = content.split("\n");
  const chunks: Omit<MarkdownChunk, "renderedHtml" | "hash">[] = [];
  let currentChunk: string[] = [];
  let chunkStartLine = 0;
  let chunkId = 0;
  let currentVisualLine = 0; // Track visual line counter

  // Helper to calculate visual lines for a string line
  const calculateVisualLines = (line: string): number => {
    if (line.length === 0) return 1; // Empty lines take 1 visual line
    return Math.ceil(line.length / charsPerLine);
  };

  let chunkVisualStartLine = 0; // Track where current chunk started visually

  const flushChunk = (endLine: number) => {
    if (currentChunk.length > 0) {
      const chunkContent = currentChunk.join("\n");

      chunks.push({
        id: `chunk-${chunkId++}`,
        startLine: chunkStartLine,
        endLine,
        visualStartLine: chunkVisualStartLine,
        visualEndLine: currentVisualLine - 1,
        content: chunkContent
      });
      currentChunk = [];
      chunkVisualStartLine = currentVisualLine; // Next chunk starts where this one ended
    }
  };

  let inCodeBlock = false;
  let inMathBlock = false;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmedLine = line.trim();

    // Track code block boundaries
    if (trimmedLine.startsWith("```")) {
      inCodeBlock = !inCodeBlock;
      currentChunk.push(line);
      currentVisualLine += calculateVisualLines(line);
      if (!inCodeBlock) {
        // End of code block - flush chunk
        flushChunk(i);
        chunkStartLine = i + 1;
      }
      continue;
    }

    // Track math block boundaries
    if (trimmedLine.startsWith("$$")) {
      inMathBlock = !inMathBlock;
      currentChunk.push(line);
      currentVisualLine += calculateVisualLines(line);
      if (!inMathBlock) {
        // End of math block - flush chunk
        flushChunk(i);
        chunkStartLine = i + 1;
      }
      continue;
    }

    // Don't split inside code or math blocks
    if (inCodeBlock || inMathBlock) {
      currentChunk.push(line);
      currentVisualLine += calculateVisualLines(line);
      continue;
    }

    // Empty line - potential chunk boundary
    if (trimmedLine === "") {
      currentChunk.push(line);
      currentVisualLine += calculateVisualLines(line);
      // Only flush if the next line starts a new block
      if (i + 1 < lines.length) {
        const nextLine = lines[i + 1].trim();
        if (
          nextLine.startsWith("#") ||
          nextLine.startsWith("-") ||
          nextLine.startsWith("*") ||
          nextLine.startsWith(">") ||
          nextLine.startsWith("1.") ||
          nextLine === ""
        ) {
          flushChunk(i);
          chunkStartLine = i + 1;
        }
      }
      continue;
    }

    // Header - start new chunk
    if (trimmedLine.startsWith("#")) {
      if (currentChunk.length > 0) {
        flushChunk(i - 1);
        chunkStartLine = i;
      }
      currentChunk.push(line);
      currentVisualLine += calculateVisualLines(line);
      continue;
    }

    // List item - check if we should start a new chunk
    if (
      trimmedLine.startsWith("-") ||
      trimmedLine.startsWith("*") ||
      /^\d+\./.test(trimmedLine)
    ) {
      // If previous chunk wasn't a list, start new chunk
      if (currentChunk.length > 0 && chunkStartLine < i) {
        const prevNonEmptyLine = currentChunk
          .slice()
          .reverse()
          .find((l: string) => l.trim() !== "");
        if (
          prevNonEmptyLine &&
          !prevNonEmptyLine.trim().startsWith("-") &&
          !prevNonEmptyLine.trim().startsWith("*") &&
          !/^\d+\./.test(prevNonEmptyLine.trim())
        ) {
          flushChunk(i - 1);
          chunkStartLine = i;
        }
      }
      currentChunk.push(line);
      currentVisualLine += calculateVisualLines(line);
      continue;
    }

    // Blockquote - start new chunk
    if (trimmedLine.startsWith(">")) {
      if (currentChunk.length > 0) {
        flushChunk(i - 1);
        chunkStartLine = i;
      }
      currentChunk.push(line);
      currentVisualLine += calculateVisualLines(line);
      continue;
    }

    // Regular line
    currentChunk.push(line);
    currentVisualLine += calculateVisualLines(line);
  }

  // Flush remaining chunk
  flushChunk(lines.length - 1);

  return chunks;
}

const ChunkRenderer = React.memo(({ chunk }: { chunk: MarkdownChunk }) => {
  return (
    <div
      className="part max-w-full overflow-wrap-anywhere [&_*]:max-w-full [&_*]:overflow-wrap-anywhere [&_table]:overflow-x-auto [&_table]:max-w-full [&_pre]:overflow-x-auto [&_code]:break-all"
      data-startline={chunk.startLine + 1}
      data-endline={chunk.endLine + 1}
      dangerouslySetInnerHTML={{ __html: chunk.renderedHtml }}
    />
  );
});

ChunkRenderer.displayName = "ChunkRenderer";

export function useChunkedMarkdown(
  content: string,
  textareaRef?: React.RefObject<HTMLTextAreaElement | null>
) {
  const md = useMarkdownRenderer();
  const previousChunksRef = useRef<MarkdownChunk[]>([]);

  // Measure textarea character width
  const charsPerLine = useMemo(() => {
    if (textareaRef?.current) {
      return measureTextareaCharWidth(textareaRef.current);
    }
    return 80; // Fallback default
  }, [textareaRef?.current?.clientWidth, textareaRef]);

  // Split content and render chunks
  const chunks = useMemo(() => {
    const newChunkSpecs = splitIntoChunks(content, charsPerLine);
    const previousChunks = previousChunksRef.current;
    const newChunks: MarkdownChunk[] = [];

    for (const spec of newChunkSpecs) {
      const hash = hashString(spec.content);

      // Try to find existing chunk with same content
      const existingChunk = previousChunks.find(
        (chunk) => chunk.hash === hash && chunk.content === spec.content
      );

      if (existingChunk) {
        // Reuse existing rendered chunk but update all position data
        newChunks.push({
          ...existingChunk,
          id: spec.id,
          startLine: spec.startLine,
          endLine: spec.endLine,
          visualStartLine: spec.visualStartLine,
          visualEndLine: spec.visualEndLine
        });
      } else {
        // Render new chunk
        const renderedHtml = renderMarkdownToHtml(md, spec.content);
        newChunks.push({
          ...spec,
          renderedHtml,
          hash
        });
      }
    }

    previousChunksRef.current = newChunks;
    return newChunks;
  }, [content, md, charsPerLine]);

  // Create JSX elements for chunks
  const chunkElements = useMemo(() => {
    if (chunks.length === 0) {
      return (
        <div className="text-muted-foreground">
          Nothing to preview yet. Start typing to see your markdown rendered
          here.
        </div>
      );
    }

    return (
      <>
        {chunks.map((chunk) => (
          <ChunkRenderer key={chunk.id} chunk={chunk} />
        ))}
      </>
    );
  }, [chunks]);

  return {
    chunkElements,
    chunks,
    charsPerLine
  };
}
