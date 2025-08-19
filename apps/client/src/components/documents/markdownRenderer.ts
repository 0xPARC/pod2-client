import MarkdownIt from "markdown-it";
import anchor from "markdown-it-anchor";
import hljs from "markdown-it-highlightjs";
import markdownItMathjax from "markdown-it-mathjax3";
import { useMemo } from "react";

// Per-render state carried via markdown-it env
type RenderState = { counter: number; insideContainer: boolean };
type RenderEnv = { __blockState?: RenderState } & Record<string, any>;

function getState(env: RenderEnv): RenderState {
  if (!env.__blockState) {
    env.__blockState = { counter: 0, insideContainer: false };
  }
  return env.__blockState;
}

// Reusable markdown-it instance creator with consistent configuration
export function useMarkdownRenderer() {
  return useMemo(() => {
    const mdInstance = new MarkdownIt({
      html: false, // Disable raw HTML for security
      xhtmlOut: false,
      breaks: true, // Convert '\n' in paragraphs into <br>
      langPrefix: "language-", // CSS language prefix for fenced blocks
      linkify: true, // Autoconvert URL-like text to links
      typographer: true // Enable smartquotes and other typographic replacements
    })
      .use(anchor, {
        // Generate heading anchors automatically
        permalink: false, // Just generate IDs, no permalink symbols
        level: [1, 2, 3, 4, 5, 6], // Generate anchors for all heading levels
        slugify: function (s: string) {
          // Create URL-friendly slugs from heading text
          const slug = s
            .toLowerCase()
            .replace(/[^a-z0-9]/g, "-")
            .replace(/-+/g, "-")
            .replace(/^-|-$/g, "");
          return slug;
        }
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

    // Helper function to add block indexing attributes
    function addBlockIndex(
      tokens: any[],
      idx: number,
      env: RenderEnv,
      forceAdd: boolean = false
    ) {
      const token = tokens[idx];
      const state = getState(env);
      // Only add index if we're not inside a container (or if forced for container blocks)
      if (token && token.attrSet && (!state.insideContainer || forceAdd)) {
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
      addBlockIndex(tokens, idx, env as RenderEnv, true); // Force add for container blocks
      // Mark that we're inside a container
      getState(env as RenderEnv).insideContainer = true;
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
      // Reset container flag
      getState(_env as RenderEnv).insideContainer = false;
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
      addBlockIndex(tokens, idx, _env as RenderEnv, true);
      getState(_env as RenderEnv).insideContainer = true;
      return renderer.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.bullet_list_close = function (
      tokens,
      idx,
      options,
      _env,
      renderer
    ) {
      getState(_env as RenderEnv).insideContainer = false;
      return renderer.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.ordered_list_open = function (
      tokens,
      idx,
      options,
      _env,
      renderer
    ) {
      addBlockIndex(tokens, idx, _env as RenderEnv, true);
      getState(_env as RenderEnv).insideContainer = true;
      return renderer.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.ordered_list_close = function (
      tokens,
      idx,
      options,
      _env,
      renderer
    ) {
      getState(_env as RenderEnv).insideContainer = false;
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
      addBlockIndex(tokens, idx, env as RenderEnv, true);
      getState(env as RenderEnv).insideContainer = true;
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
      getState(env as RenderEnv).insideContainer = false;
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
      const blockIndex =
        token.attrs?.find((attr) => attr[0] === "data-block-index")?.[1] || "0";
      return `<div data-block-index="${blockIndex}">${mathHtml}</div>`;
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

  // Track which block-level tokens we care about
  const blockTokenTypes = new Set([
    "paragraph_open",
    "heading_open",
    "blockquote_open",
    "bullet_list_open",
    "ordered_list_open",
    "fence",
    "code_block",
    "hr",
    "table_open",
    "math_block",
    "html_block"
  ]);

  let i = 0;
  while (i < tokens.length) {
    const token = tokens[i];

    // Skip non-block tokens
    if (!blockTokenTypes.has(token.type)) {
      i++;
      continue;
    }

    // Get the content for this block
    let blockContent = "";
    let endTokenType = "";

    // Determine the closing token type and extract content
    switch (token.type) {
      case "paragraph_open":
        endTokenType = "paragraph_close";
        break;
      case "heading_open":
        endTokenType = "heading_close";
        break;
      case "blockquote_open":
        endTokenType = "blockquote_close";
        break;
      case "bullet_list_open":
        endTokenType = "bullet_list_close";
        break;
      case "ordered_list_open":
        endTokenType = "ordered_list_close";
        break;
      case "table_open":
        endTokenType = "table_close";
        break;
      case "fence":
      case "code_block":
      case "math_block":
      case "html_block":
        // These are self-contained tokens with content
        if (token.content) {
          // For fenced code blocks, include the fence markers
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
      case "hr":
        // Horizontal rules are self-contained
        blocks.push("---");
        i++;
        continue;
    }

    // For container tokens, extract content from source lines
    if (endTokenType && token.map) {
      const [startLine, endLine] = token.map;
      const blockLines = lines.slice(startLine, endLine);
      blockContent = blockLines.join("\n");

      if (blockContent.trim()) {
        blocks.push(blockContent.trim());
      }

      // Skip to the closing token
      while (i < tokens.length && tokens[i].type !== endTokenType) {
        i++;
      }
    }

    i++;
  }

  return blocks;
}
