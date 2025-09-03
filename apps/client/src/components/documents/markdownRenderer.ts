import MarkdownIt from "markdown-it";
import {
  createBaseMarkdownIt,
  sanitizeFenceInfo
} from "../../lib/markdown/setup";
import { useMemo } from "react";

// Per-render state carried via markdown-it env
// Track a nesting depth for container blocks so we only
// assign indices to top-level blocks/containers.
type RenderState = { counter: number; containerDepth: number };
type RenderEnv = { __blockState?: RenderState } & Record<string, any>;

function getState(env: RenderEnv): RenderState {
  if (!env.__blockState) {
    env.__blockState = { counter: 0, containerDepth: 0 };
  }
  return env.__blockState;
}

// Reusable markdown-it instance creator with consistent configuration
export function useMarkdownRenderer() {
  return useMemo(() => {
    const mdInstance = createBaseMarkdownIt();

    // Helper function to add block indexing attributes
    function addBlockIndex(tokens: any[], idx: number, env: RenderEnv) {
      const token = tokens[idx];
      const state = getState(env);
      // Only add index if we're not inside any container depth
      if (token && token.attrSet && state.containerDepth === 0) {
        token.attrSet("data-block-index", state.counter.toString());
        state.counter++;
      }
    }

    // Store original renderers
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

    // Override renderers to add block indexing
    mdInstance.renderer.rules.paragraph_open = function (
      tokens,
      idx,
      options,
      env,
      renderer
    ) {
      addBlockIndex(tokens, idx, env as RenderEnv);
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
      addBlockIndex(tokens, idx, env as RenderEnv);
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
      // Add index only for top-level container, then increase depth
      addBlockIndex(tokens, idx, env as RenderEnv);
      getState(env as RenderEnv).containerDepth++;
      return originalRules.blockquote_open
        ? originalRules.blockquote_open(tokens, idx, options, env, renderer)
        : renderer.renderToken(tokens, idx, options);
    };

    // Add blockquote_close handler to reset container flag
    mdInstance.renderer.rules.blockquote_close = function (
      tokens,
      idx,
      options,
      _env,
      renderer
    ) {
      // Decrease container depth on close
      const state = getState(_env as RenderEnv);
      state.containerDepth = Math.max(0, state.containerDepth - 1);
      return renderer.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.code_block = function (
      tokens,
      idx,
      options,
      env,
      renderer
    ) {
      addBlockIndex(tokens, idx, env as RenderEnv);
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
      addBlockIndex(tokens, idx, env as RenderEnv);
      // Sanitize code fence language to avoid hljs errors
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
      addBlockIndex(tokens, idx, env as RenderEnv);
      return originalRules.hr
        ? originalRules.hr(tokens, idx, options, env, renderer)
        : renderer.renderToken(tokens, idx, options);
    };

    // For lists, we want to track the list itself, not individual items
    mdInstance.renderer.rules.bullet_list_open = function (
      tokens,
      idx,
      options,
      _env,
      renderer
    ) {
      addBlockIndex(tokens, idx, _env as RenderEnv);
      getState(_env as RenderEnv).containerDepth++;
      return renderer.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.bullet_list_close = function (
      tokens,
      idx,
      options,
      _env,
      renderer
    ) {
      const state = getState(_env as RenderEnv);
      state.containerDepth = Math.max(0, state.containerDepth - 1);
      return renderer.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.ordered_list_open = function (
      tokens,
      idx,
      options,
      _env,
      renderer
    ) {
      addBlockIndex(tokens, idx, _env as RenderEnv);
      getState(_env as RenderEnv).containerDepth++;
      return renderer.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.ordered_list_close = function (
      tokens,
      idx,
      options,
      _env,
      renderer
    ) {
      const state = getState(_env as RenderEnv);
      state.containerDepth = Math.max(0, state.containerDepth - 1);
      return renderer.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.list_item_open = function (
      tokens,
      idx,
      options,
      env,
      renderer
    ) {
      // Don't add index to list items - they're inside a container
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
      // Treat table as a container block
      addBlockIndex(tokens, idx, env as RenderEnv);
      getState(env as RenderEnv).containerDepth++;
      return originalRules.table_open
        ? originalRules.table_open(tokens, idx, options, env, renderer)
        : renderer.renderToken(tokens, idx, options);
    };

    // Close handler for table container
    mdInstance.renderer.rules.table_close = function (
      tokens,
      idx,
      options,
      env,
      renderer
    ) {
      const state = getState(env as RenderEnv);
      state.containerDepth = Math.max(0, state.containerDepth - 1);
      return renderer.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.math_block = function (
      tokens,
      idx,
      options,
      env,
      renderer
    ) {
      addBlockIndex(tokens, idx, env as RenderEnv);
      const mathHtml = originalRules.math_block
        ? originalRules.math_block(tokens, idx, options, env, renderer)
        : renderer.renderToken(tokens, idx, options);

      // Wrap math block in div with block index
      const token = tokens[idx];
      const blockIndex = token.attrs?.find(
        (attr) => attr[0] === "data-block-index"
      )?.[1];
      const attr = blockIndex ? ` data-block-index="${blockIndex}"` : "";
      return `<div${attr}>${mathHtml}</div>`;
    };

    // link_open handler is applied by createBaseMarkdownIt

    return mdInstance;
  }, []);
}

// Render markdown content to HTML string
export function renderMarkdownToHtml(md: MarkdownIt, content: string): string {
  if (!content.trim()) {
    return "";
  }
  // Use isolated per-render env state
  const env: RenderEnv = {};
  return md.render(content, env);
}

// Enhanced render function that returns both HTML and block mapping
export function renderMarkdownWithBlocks(
  md: MarkdownIt,
  content: string
): { html: string; blocks: string[] } {
  if (!content.trim()) {
    return { html: "", blocks: [] };
  }

  // Use MarkdownIt's parse method to get tokens
  const tokens = md.parse(content, {});
  const blocks = extractBlocksFromTokens(tokens, content);

  // env will be used to track block indices within the rendering process
  const env: RenderEnv = {};
  const html = md.render(content, env);

  return { html, blocks };
}

// Extract block content from MarkdownIt tokens
function extractBlocksFromTokens(tokens: any[], content: string): string[] {
  const lines = content.split("\n");
  const blocks: string[] = [];

  // Block-level tokens we care about
  const containerOpens = new Set([
    "blockquote_open",
    "bullet_list_open",
    "ordered_list_open",
    "table_open"
  ]);
  const containerCloses = new Set([
    "blockquote_close",
    "bullet_list_close",
    "ordered_list_close",
    "table_close"
  ]);

  const blockTokenTypes = new Set([
    "paragraph_open",
    "heading_open",
    ...Array.from(containerOpens),
    "fence",
    "code_block",
    "hr",
    "math_block",
    "html_block"
  ]);

  let i = 0;
  let depth = 0; // container nesting depth

  while (i < tokens.length) {
    const token = tokens[i];

    // Maintain depth for containers
    if (containerOpens.has(token.type)) {
      if (depth === 0) {
        // At top level: extract this entire container as one block
        let blockContent = "";
        const endTokenType = token.type.replace("_open", "_close");

        if (token.map) {
          const [startLine, endLine] = token.map;
          const blockLines = lines.slice(startLine, endLine);
          blockContent = blockLines.join("\n");
          if (blockContent.trim()) {
            blocks.push(blockContent.trim());
          }
        }

        // Advance to matching close, tracking nested containers of any type
        let localDepth = 1;
        i++;
        while (i < tokens.length && localDepth > 0) {
          if (containerOpens.has(tokens[i].type)) localDepth++;
          else if (tokens[i].type === endTokenType) localDepth--;
          i++;
        }
        // We've advanced past the close; continue without further increment
        continue;
      } else {
        // Nested container: just bump depth and move on
        depth++;
        i++;
        continue;
      }
    }

    if (containerCloses.has(token.type)) {
      depth = Math.max(0, depth - 1);
      i++;
      continue;
    }

    // Skip non-block tokens
    if (!blockTokenTypes.has(token.type)) {
      i++;
      continue;
    }

    // Only capture non-container blocks when not inside a container
    if (depth > 0) {
      i++;
      continue;
    }

    switch (token.type) {
      case "paragraph_open":
      case "heading_open": {
        if (token.map) {
          const [startLine, endLine] = token.map;
          const blockLines = lines.slice(startLine, endLine);
          const blockContent = blockLines.join("\n");
          if (blockContent.trim()) blocks.push(blockContent.trim());
        }
        // Skip ahead to the corresponding close token
        const endTokenType = token.type.replace("_open", "_close");
        i++;
        while (i < tokens.length && tokens[i].type !== endTokenType) i++;
        i++;
        continue;
      }
      case "fence":
      case "code_block":
      case "math_block":
      case "html_block": {
        if (token.content) {
          let blockContent = "";
          if (token.type === "fence") {
            const info = token.info || "";
            blockContent = "```" + info + "\n" + token.content + "```";
          } else if (token.type === "math_block") {
            blockContent = "$$\n" + token.content + "$$";
          } else {
            blockContent = token.content;
          }
          blocks.push(blockContent.trim());
        }
        i++;
        continue;
      }
      case "hr": {
        blocks.push("---");
        i++;
        continue;
      }
    }

    i++;
  }

  return blocks;
}
