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

// Message types for worker communication
export interface MarkdownRenderRequest {
  type: "render";
  markdown: string;
  sequenceId: number;
  sharedBuffer?: SharedArrayBuffer;
}

export interface BlockMapping {
  startLine: number;
  endLine: number;
  elementType: string;
  elementIndex: number;
}

export interface MarkdownRenderResponse {
  type: "render-complete";
  html: string;
  blockMappings: BlockMapping[];
  sequenceId: number;
}

export interface MarkdownErrorResponse {
  type: "error";
  error: string;
  sequenceId: number;
}

export type MarkdownWorkerMessage = MarkdownRenderRequest;
export type MarkdownWorkerResponse =
  | MarkdownRenderResponse
  | MarkdownErrorResponse;

// Global variable to collect block mappings during rendering
let blockMappings: BlockMapping[] = [];
let elementCounter = 0;

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
    table_open: mdInstance.renderer.rules.table_open
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
    const { type, markdown, sequenceId, sharedBuffer } = event.data;

    if (type !== "render") {
      return;
    }

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

      // Check again if this request is still valid (in case a newer one arrived during rendering)
      if (sharedBuffer) {
        const sharedArray = new Int32Array(sharedBuffer);
        const latestSequenceId = Atomics.load(sharedArray, 0);

        // If a newer request has been sent while we were rendering, discard this result
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
);

// Export types for main thread (this won't be executed in worker context)
export {};
