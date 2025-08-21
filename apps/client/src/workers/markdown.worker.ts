// @ts-nocheck
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
import { createBaseMarkdownIt, sanitizeFenceInfo } from "../lib/markdown/setup";

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

export interface MarkdownChangeEvent {
  type: "change-event";
  change: MonacoChange;
  fullText: string;
}

export interface MarkdownInitEvent {
  type: "init-message";
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

export interface MarkdownIncrementalResponse {
  type: "incremental-complete";
  html: string;
  blockMappings: BlockMapping[];
  affectedRegions: AffectedRegion[];
}

export interface MarkdownErrorResponse {
  type: "error";
  error: string;
}

export type MarkdownWorkerMessage = MarkdownChangeEvent | MarkdownInitEvent;
export type MarkdownWorkerResponse =
  | MarkdownIncrementalResponse
  | MarkdownErrorResponse;

// Global variable to collect block mappings during rendering
let blockMappings: BlockMapping[] = [];
let elementCounter = 0;

// Worker state machine
type WorkerState = "idle" | "busy" | "collecting";
let workerState: WorkerState = "idle";
let messageQueue: MarkdownChangeEvent[] = [];

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
        changeType: "modify"
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

// Process change events (can be single change or multiple merged changes)
function processChangeEvents(changeEvents: MarkdownChangeEvent[]) {
  try {
    // Get the text from the last change (most up-to-date content)
    const finalText = changeEvents[changeEvents.length - 1].fullText;

    // Merge all changes into affected regions for optimal DOM updates
    const affectedRegions = mergeChanges(changeEvents);

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
      affectedRegions
    };

    self.postMessage(response);
  } catch (error) {
    // Send error response
    const errorResponse: MarkdownErrorResponse = {
      type: "error",
      error: error instanceof Error ? error.message : String(error)
    };

    self.postMessage(errorResponse);
  }
}

// Handle work completion and state transitions
function handleWorkComplete() {
  if (workerState !== "collecting") {
    // Shouldn't happen, but handle gracefully
    workerState = "idle";
    return;
  }

  if (messageQueue.length === 0) {
    // No queued messages, return to idle
    workerState = "idle";
  } else {
    // Process all queued messages together for better affected regions
    const queuedMessages = [...messageQueue]; // Copy the queue
    messageQueue = []; // Clear the queue

    // Transition to busy and process all messages
    workerState = "busy";
    processChangeEvents(queuedMessages);

    // Transition to collecting and schedule work complete handler
    workerState = "collecting";
    setTimeout(handleWorkComplete, 0);
  }
}

// Create markdown-it instance with same configuration as main thread plus line mapping
function createMarkdownRenderer(): MarkdownIt {
  const mdInstance = createBaseMarkdownIt();

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
    const token = tokens[idx];
    token.info = sanitizeFenceInfo(token.info);

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

    // Render math using the MathJax3 renderer
    const mathHtml = originalRules.math_block!(
      tokens,
      idx,
      options,
      env,
      renderer
    );

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

  // link_open handler is applied by createBaseMarkdownIt

  return mdInstance;
}

// Initialize markdown renderer
const md = createMarkdownRenderer();

// Handle messages from main thread
self.addEventListener(
  "message",
  (event: MessageEvent<MarkdownWorkerMessage>) => {
    const messageData = event.data;

    if (messageData.type === "init-message") {
      // Worker initialization - just send back a simple response to mark as ready
      const response: MarkdownIncrementalResponse = {
        type: "incremental-complete",
        html: "",
        blockMappings: [],
        affectedRegions: []
      };
      self.postMessage(response);
    } else if (messageData.type === "change-event") {
      const changeEvent = messageData as MarkdownChangeEvent;

      switch (workerState) {
        case "idle":
          // Process immediately and transition to busy
          workerState = "busy";
          processChangeEvents([changeEvent]);

          // Transition to collecting and schedule work complete handler
          workerState = "collecting";
          setTimeout(handleWorkComplete, 0);
          break;

        case "busy":
          // Currently processing, this message will be handled in collecting phase
          break;

        case "collecting":
          // Queue the message for later processing
          messageQueue.push(changeEvent);
          break;
      }
    }
  }
);

// Export types for main thread (this won't be executed in worker context)
export {};
