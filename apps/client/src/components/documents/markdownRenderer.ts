import MarkdownIt from "markdown-it";
import anchor from "markdown-it-anchor";
import hljs from "markdown-it-highlightjs";
import markdownItMathjax from "markdown-it-mathjax3";
import { useMemo } from "react";

// Global variables to track block indices during rendering
let blockCounter = 0;
let insideContainer = false; // Track if we're inside a container block

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
      forceAdd: boolean = false
    ) {
      const token = tokens[idx];
      // Only add index if we're not inside a container (or if forced for container blocks)
      if (token && token.attrSet && (!insideContainer || forceAdd)) {
        token.attrSet("data-block-index", blockCounter.toString());
        blockCounter++;
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
      addBlockIndex(tokens, idx);
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
      addBlockIndex(tokens, idx);
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
      addBlockIndex(tokens, idx, true); // Force add for container blocks
      insideContainer = true; // Mark that we're inside a container
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
      insideContainer = false; // Reset container flag
      return renderer.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.code_block = function (
      tokens,
      idx,
      options,
      env,
      renderer
    ) {
      addBlockIndex(tokens, idx);
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
      addBlockIndex(tokens, idx);
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
      addBlockIndex(tokens, idx);
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
      addBlockIndex(tokens, idx, true);
      insideContainer = true;
      return renderer.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.bullet_list_close = function (
      tokens,
      idx,
      options,
      _env,
      renderer
    ) {
      insideContainer = false;
      return renderer.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.ordered_list_open = function (
      tokens,
      idx,
      options,
      _env,
      renderer
    ) {
      addBlockIndex(tokens, idx, true);
      insideContainer = true;
      return renderer.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.ordered_list_close = function (
      tokens,
      idx,
      options,
      _env,
      renderer
    ) {
      insideContainer = false;
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
      addBlockIndex(tokens, idx);
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
      addBlockIndex(tokens, idx);
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
  // Reset block counter and container flag for each render
  blockCounter = 0;
  insideContainer = false;
  return md.render(content);
}

// Enhanced render function that returns both HTML and block mapping
export function renderMarkdownWithBlocks(
  md: MarkdownIt,
  content: string
): { html: string; blocks: string[] } {
  if (!content.trim()) {
    return { html: "", blocks: [] };
  }

  // Parse markdown into blocks
  const blocks = parseMarkdownBlocks(content);

  // Reset block counter and container flag, then render
  blockCounter = 0;
  insideContainer = false;
  const html = md.render(content);

  return { html, blocks };
}

// Parse markdown content into individual blocks
function parseMarkdownBlocks(content: string): string[] {
  const lines = content.split("\n");
  const blocks: string[] = [];
  let currentBlock = "";
  let inCodeBlock = false;
  let inMathBlock = false;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    // Track code block boundaries
    if (line.startsWith("```")) {
      inCodeBlock = !inCodeBlock;
      currentBlock += line + "\n";
      if (!inCodeBlock) {
        // End of code block
        blocks.push(currentBlock.trim());
        currentBlock = "";
      }
      continue;
    }

    // Track math block boundaries
    if (line.startsWith("$$")) {
      inMathBlock = !inMathBlock;
      currentBlock += line + "\n";
      if (!inMathBlock) {
        // End of math block
        blocks.push(currentBlock.trim());
        currentBlock = "";
      }
      continue;
    }

    // If we're inside a code or math block, just accumulate
    if (inCodeBlock || inMathBlock) {
      currentBlock += line + "\n";
      continue;
    }

    // Empty line - end current block if it exists
    if (line.trim() === "") {
      if (currentBlock.trim()) {
        blocks.push(currentBlock.trim());
        currentBlock = "";
      }
      continue;
    }

    // Check if this line starts a new block type
    const isBlockStart =
      line.startsWith("#") || // Heading
      line.startsWith("- ") || // List item
      line.startsWith("* ") || // List item
      line.match(/^\d+\. /) || // Numbered list
      line.startsWith("> ") || // Blockquote
      line.startsWith("---") || // HR
      line.startsWith("| "); // Table

    // If we have accumulated content and this is a new block start, save the previous block
    if (isBlockStart && currentBlock.trim()) {
      blocks.push(currentBlock.trim());
      currentBlock = line + "\n";
    } else {
      currentBlock += line + "\n";
    }
  }

  // Add any remaining block
  if (currentBlock.trim()) {
    blocks.push(currentBlock.trim());
  }

  return blocks;
}
