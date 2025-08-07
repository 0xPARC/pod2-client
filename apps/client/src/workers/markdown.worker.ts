// Markdown Web Worker
// Handles markdown-to-HTML rendering off the main thread for better performance

/// <reference lib="webworker" />

// Worker environment shim to handle Vite HMR and global variables
if (
  typeof WorkerGlobalScope !== "undefined" &&
  self instanceof WorkerGlobalScope
) {
  (self as any).global = self;
  (self as any).window = self;
}

// Disable React Fast Refresh in worker context
if (typeof (self as any).$RefreshReg$ === "undefined") {
  (self as any).$RefreshReg$ = () => {};
  (self as any).$RefreshSig$ = () => (type: any) => type;
  (self as any).__vite_plugin_react_preamble_installed__ = true;
}

import MarkdownIt from "markdown-it";
import hljs from "markdown-it-highlightjs";
import markdownItMathjax from "markdown-it-mathjax3";

// Monaco Editor change information
export interface MonacoChange {
  range: {
    startLineNumber: number;
    startColumn: number;
    endLineNumber: number;
    endColumn: number;
  };
  rangeLength: number;
  text: string;
}

// Message types for worker communication
export interface MarkdownRenderRequest {
  type: "render";
  markdown: string;
  sequenceId: number;
  sharedBuffer?: SharedArrayBuffer;
}

export interface MarkdownChangeEvent {
  type: "change-event";
  change: MonacoChange;
  fullText: string;
  sequenceId: number;
  latestSequenceId: number;
}

export interface BlockMapping {
  startLine: number;
  endLine: number;
  elementType: string;
  elementIndex: number;
}

export interface AffectedRegion {
  startLine: number;
  endLine: number;
  changeType: "insert" | "delete" | "modify";
}

export interface MarkdownRenderResponse {
  type: "render-complete";
  html: string;
  blockMappings: BlockMapping[];
  sequenceId: number;
}

export interface MarkdownIncrementalResponse {
  type: "incremental-complete";
  html: string;
  blockMappings: BlockMapping[];
  affectedRegions: AffectedRegion[];
  sequenceId: number;
}

export interface MarkdownErrorResponse {
  type: "error";
  error: string;
  sequenceId: number;
}

export type MarkdownWorkerMessage = MarkdownRenderRequest | MarkdownChangeEvent;
export type MarkdownWorkerResponse =
  | MarkdownRenderResponse
  | MarkdownIncrementalResponse
  | MarkdownErrorResponse;

// Global variable to collect block mappings during rendering
let blockMappings: BlockMapping[] = [];
let elementCounter = 0;

// Change batching state
let pendingChanges: MarkdownChangeEvent[] = [];
let isProcessing = false;

// Helper function to merge overlapping changes into affected regions
function mergeChanges(changes: MarkdownChangeEvent[]): AffectedRegion[] {
  if (changes.length === 0) return [];

  // Collect all affected lines
  const affectedLines = new Set<number>();

  for (const changeEvent of changes) {
    const change = changeEvent.change;
    const startLine = change.range.startLineNumber - 1; // Convert to 0-indexed
    const endLine = change.range.endLineNumber - 1;

    for (let line = startLine; line <= endLine; line++) {
      affectedLines.add(line);
    }
  }

  // Convert to sorted array and consolidate into contiguous regions
  const sortedLines = Array.from(affectedLines).sort((a, b) => a - b);
  const regions: AffectedRegion[] = [];

  if (sortedLines.length === 0) return regions;

  let regionStart = sortedLines[0];
  let regionEnd = sortedLines[0];

  for (let i = 1; i < sortedLines.length; i++) {
    const line = sortedLines[i];

    if (line === regionEnd + 1) {
      // Extend current region
      regionEnd = line;
    } else {
      // Start new region
      regions.push({
        startLine: regionStart,
        endLine: regionEnd,
        changeType: "modify" // Simplified - could be more sophisticated
      });
      regionStart = line;
      regionEnd = line;
    }
  }

  // Add final region
  regions.push({
    startLine: regionStart,
    endLine: regionEnd,
    changeType: "modify"
  });

  return regions;
}

// Process accumulated changes
function processAccumulatedChanges() {
  if (pendingChanges.length === 0) return;

  isProcessing = true;

  try {
    const changesToProcess = pendingChanges;
    pendingChanges = []; // Clear queue

    // Get the final text from the last change
    const finalText = changesToProcess[changesToProcess.length - 1].fullText;
    const finalSequenceId =
      changesToProcess[changesToProcess.length - 1].sequenceId;

    // Merge changes into affected regions
    const affectedRegions = mergeChanges(changesToProcess);

    // Reset mapping data for this render
    blockMappings = [];
    elementCounter = 0;

    // Render markdown to HTML
    const html = finalText.trim() ? md.render(finalText) : "";

    // Send incremental response
    const response: MarkdownIncrementalResponse = {
      type: "incremental-complete",
      html,
      blockMappings: [...blockMappings], // Copy the array
      affectedRegions,
      sequenceId: finalSequenceId
    };

    self.postMessage(response);
  } catch (error) {
    // Send error response
    const errorResponse: MarkdownErrorResponse = {
      type: "error",
      error: error instanceof Error ? error.message : String(error),
      sequenceId: pendingChanges.length > 0 ? pendingChanges[0].sequenceId : 0
    };

    self.postMessage(errorResponse);
  } finally {
    isProcessing = false;

    // If more changes arrived while processing, handle them
    if (pendingChanges.length > 0) {
      // Check if the latest change is complete (no more pending)
      const latestChange = pendingChanges[pendingChanges.length - 1];
      const isLatest =
        latestChange.sequenceId === latestChange.latestSequenceId;

      if (isLatest) {
        processAccumulatedChanges();
      }
    }
  }
}

// Create markdown-it instance with same configuration as main thread plus line mapping
function createMarkdownRenderer(): MarkdownIt {
  const mdInstance = new MarkdownIt({
    html: false, // Disable raw HTML for security
    xhtmlOut: false,
    breaks: true, // Convert '\n' in paragraphs into <br>
    langPrefix: "language-", // CSS language prefix for fenced blocks
    linkify: true, // Autoconvert URL-like text to links
    typographer: true // Enable smartquotes and other typographic replacements
  })
    .use(hljs, {
      // Configure highlight.js to handle language parsing properly
      auto: false, // Disable auto-detection to avoid errors
      code: true // Only highlight code blocks with explicit language
    }) // Add syntax highlighting with error handling
    .use(markdownItMathjax, {
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
        loader: { load: ["[tex]/textmacros", "[tex]/textcomp"] },
        tex: { packages: { "[+]": ["textmacros"] } },
        textmacros: { packages: { "[+]": ["textcomp"] } },
        processEscapes: true,
        macros: {
          "\\RR": "\\mathbb{R}",
          "\\NN": "\\mathbb{N}"
        }
      }
    })
    .enable([
      "table", // GitHub tables
      "strikethrough" // ~~text~~
    ]);

  // Add line mapping to block-level elements
  const originalRules = {
    paragraph_open: mdInstance.renderer.rules.paragraph_open,
    heading_open: mdInstance.renderer.rules.heading_open,
    blockquote_open: mdInstance.renderer.rules.blockquote_open,
    code_block: mdInstance.renderer.rules.code_block,
    fence: mdInstance.renderer.rules.fence,
    hr: mdInstance.renderer.rules.hr,
    list_item_open: mdInstance.renderer.rules.list_item_open,
    table_open: mdInstance.renderer.rules.table_open,
    math_block: mdInstance.renderer.rules.math_block
  };

  // Helper function to add line mapping attributes
  function addLineMappingAttributes(
    tokens: any[],
    idx: number,
    elementType: string
  ) {
    const token = tokens[idx];
    if (token.map) {
      const startLine = token.map[0];
      const endLine = token.map[1] - 1; // markdown-it uses exclusive end
      const currentElementIndex = elementCounter++;

      // Add data attributes
      token.attrSet("data-md-line-start", startLine.toString());
      token.attrSet("data-md-line-end", endLine.toString());
      token.attrSet("data-md-element-index", currentElementIndex.toString());

      // Store mapping for later use
      blockMappings.push({
        startLine,
        endLine,
        elementType,
        elementIndex: currentElementIndex
      });
    }
  }

  // Override renderers for block elements
  mdInstance.renderer.rules.paragraph_open = function (
    tokens,
    idx,
    options,
    env,
    renderer
  ) {
    addLineMappingAttributes(tokens, idx, "paragraph");
    return originalRules.paragraph_open
      ? originalRules.paragraph_open(tokens, idx, options, env, renderer)
      : renderer.renderToken(tokens, idx, options);
  };

  mdInstance.renderer.rules.heading_open = function (
    tokens,
    idx,
    options,
    env,
    renderer
  ) {
    addLineMappingAttributes(tokens, idx, "heading");
    return originalRules.heading_open
      ? originalRules.heading_open(tokens, idx, options, env, renderer)
      : renderer.renderToken(tokens, idx, options);
  };

  mdInstance.renderer.rules.blockquote_open = function (
    tokens,
    idx,
    options,
    env,
    renderer
  ) {
    addLineMappingAttributes(tokens, idx, "blockquote");
    return originalRules.blockquote_open
      ? originalRules.blockquote_open(tokens, idx, options, env, renderer)
      : renderer.renderToken(tokens, idx, options);
  };

  mdInstance.renderer.rules.code_block = function (
    tokens,
    idx,
    options,
    env,
    renderer
  ) {
    addLineMappingAttributes(tokens, idx, "code_block");
    return originalRules.code_block
      ? originalRules.code_block(tokens, idx, options, env, renderer)
      : renderer.renderToken(tokens, idx, options);
  };

  mdInstance.renderer.rules.fence = function (
    tokens,
    idx,
    options,
    env,
    renderer
  ) {
    addLineMappingAttributes(tokens, idx, "fence");

    // Clean up language identifier to avoid highlightjs errors
    const token = tokens[idx];
    if (token.info) {
      // Extract just the language name, ignoring attributes
      let lang = token.info.trim();

      // Handle pandoc-style attributes like `python {.numberLines frame="single"}`
      const spaceIndex = lang.indexOf(" ");
      if (spaceIndex > 0) {
        lang = lang.substring(0, spaceIndex);
      }

      // Handle attribute-only cases like `{frame="single"}` or `{.numberLines`
      if (lang.startsWith("{") || lang.includes("=") || lang.includes('"')) {
        lang = ""; // Clear invalid language
      }

      // Clean up dotted class names like `.python` -> `python`
      if (lang.startsWith(".")) {
        lang = lang.substring(1);
      }

      // Update the token with cleaned language
      token.info = lang;
    }

    return originalRules.fence
      ? originalRules.fence(tokens, idx, options, env, renderer)
      : renderer.renderToken(tokens, idx, options);
  };

  mdInstance.renderer.rules.hr = function (
    tokens,
    idx,
    options,
    env,
    renderer
  ) {
    addLineMappingAttributes(tokens, idx, "hr");
    return originalRules.hr
      ? originalRules.hr(tokens, idx, options, env, renderer)
      : renderer.renderToken(tokens, idx, options);
  };

  mdInstance.renderer.rules.list_item_open = function (
    tokens,
    idx,
    options,
    env,
    renderer
  ) {
    addLineMappingAttributes(tokens, idx, "list_item");
    return originalRules.list_item_open
      ? originalRules.list_item_open(tokens, idx, options, env, renderer)
      : renderer.renderToken(tokens, idx, options);
  };

  mdInstance.renderer.rules.table_open = function (
    tokens,
    idx,
    options,
    env,
    renderer
  ) {
    addLineMappingAttributes(tokens, idx, "table");
    return originalRules.table_open
      ? originalRules.table_open(tokens, idx, options, env, renderer)
      : renderer.renderToken(tokens, idx, options);
  };

  mdInstance.renderer.rules.math_block = function (
    tokens,
    idx,
    options,
    env,
    renderer
  ) {
    addLineMappingAttributes(tokens, idx, "math_block");

    // Get the token with line mapping attributes
    const token = tokens[idx];

    // Render math using the original mathjax3 renderer (if available) or fallback
    const mathHtml = originalRules.math_block
      ? originalRules.math_block(tokens, idx, options, env, renderer)
      : renderer.renderToken(tokens, idx, options);

    // Extract attributes from the token and apply them to a wrapper div
    const attrs = token.attrs || [];
    if (attrs.length > 0) {
      const attrString = attrs
        .map(([name, value]) => `${name}="${value}"`)
        .join(" ");
      return `<div ${attrString}>${mathHtml}</div>`;
    }

    return mathHtml;
  };

  // Custom renderer for links to open in new tab
  mdInstance.renderer.rules.link_open = function (
    tokens,
    idx,
    options,
    _env,
    renderer
  ) {
    const aIndex = tokens[idx].attrIndex("target");
    if (aIndex < 0) {
      tokens[idx].attrPush(["target", "_blank"]);
      tokens[idx].attrPush(["rel", "noopener noreferrer"]);
    } else {
      tokens[idx].attrs![aIndex][1] = "_blank";
    }
    return renderer.renderToken(tokens, idx, options);
  };

  return mdInstance;
}

// Initialize markdown renderer
const md = createMarkdownRenderer();

// Handle messages from main thread
self.addEventListener(
  "message",
  (event: MessageEvent<MarkdownWorkerMessage>) => {
    const messageData = event.data;

    if (messageData.type === "change-event") {
      // Handle change events with batching
      const changeEvent = messageData as MarkdownChangeEvent;

      // Always add to batch
      pendingChanges.push(changeEvent);

      // Check if this is the latest message in the sequence
      const isLatest = changeEvent.sequenceId === changeEvent.latestSequenceId;

      if (isLatest && !isProcessing) {
        // This is the last message, and we're free to process
        processAccumulatedChanges();
      }
      // Otherwise, we wait for the next event handler to fire
    } else if (messageData.type === "render") {
      // Handle legacy render requests (for backward compatibility)
      const { markdown, sequenceId, sharedBuffer } = messageData;

      try {
        // Check if this request is still the latest (if SharedArrayBuffer is available)
        if (sharedBuffer) {
          const sharedArray = new Int32Array(sharedBuffer);
          const latestSequenceId = Atomics.load(sharedArray, 0);

          // If a newer request has been sent, discard this one
          if (sequenceId < latestSequenceId) {
            return; // Silently discard stale request
          }
        }

        // Reset mapping data for this render
        blockMappings = [];
        elementCounter = 0;

        // Render markdown to HTML
        const html = markdown.trim() ? md.render(markdown) : "";

        // Check again if this request is still valid
        if (sharedBuffer) {
          const sharedArray = new Int32Array(sharedBuffer);
          const latestSequenceId = Atomics.load(sharedArray, 0);

          if (sequenceId < latestSequenceId) {
            return; // Silently discard stale result
          }

          // Update completed sequence ID
          Atomics.store(sharedArray, 1, sequenceId);
        }

        // Send result back to main thread
        const response: MarkdownRenderResponse = {
          type: "render-complete",
          html,
          blockMappings: [...blockMappings], // Copy the array
          sequenceId
        };

        self.postMessage(response);
      } catch (error) {
        // Send error back to main thread
        const errorResponse: MarkdownErrorResponse = {
          type: "error",
          error: error instanceof Error ? error.message : String(error),
          sequenceId
        };

        self.postMessage(errorResponse);
      }
    }
  }
);

// Export types for main thread (this won't be executed in worker context)
export {};
