import MarkdownIt from "markdown-it";
import hljs from "markdown-it-highlightjs";
import markdownItMathjax from "markdown-it-mathjax3";
import { useMemo } from "react";

// Helper function to add line mapping attributes to tokens
function addLineMapping(tokens: any[], idx: number) {
  if (tokens[idx].map && tokens[idx].level === 0) {
    const startline = tokens[idx].map[0] + 1;
    const endline = tokens[idx].map[1];
    tokens[idx].attrJoin("class", "part");
    tokens[idx].attrJoin("data-startline", startline.toString());
    tokens[idx].attrJoin("data-endline", endline.toString());
  }
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
      .use(hljs) // Add syntax highlighting
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

    // Override renderer rules to add line mapping for scroll sync
    mdInstance.renderer.rules.paragraph_open = function (
      tokens,
      idx,
      options,
      _env,
      self
    ) {
      addLineMapping(tokens, idx);
      return self.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.heading_open = function (
      tokens,
      idx,
      options,
      _env,
      self
    ) {
      tokens[idx].attrJoin("class", "raw");
      addLineMapping(tokens, idx);
      return self.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.blockquote_open = function (
      tokens,
      idx,
      options,
      _env,
      self
    ) {
      tokens[idx].attrJoin("class", "raw");
      addLineMapping(tokens, idx);
      return self.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.bullet_list_open = function (
      tokens,
      idx,
      options,
      _env,
      self
    ) {
      addLineMapping(tokens, idx);
      return self.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.ordered_list_open = function (
      tokens,
      idx,
      options,
      _env,
      self
    ) {
      addLineMapping(tokens, idx);
      return self.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.list_item_open = function (
      tokens,
      idx,
      options,
      _env,
      self
    ) {
      tokens[idx].attrJoin("class", "raw");
      if (tokens[idx].map) {
        const startline = tokens[idx].map[0] + 1;
        const endline = tokens[idx].map[1];
        tokens[idx].attrJoin("data-startline", startline.toString());
        tokens[idx].attrJoin("data-endline", endline.toString());
      }
      return self.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.table_open = function (
      tokens,
      idx,
      options,
      _env,
      self
    ) {
      addLineMapping(tokens, idx);
      return self.renderToken(tokens, idx, options);
    };

    mdInstance.renderer.rules.fence = function (
      tokens,
      idx,
      options,
      _env,
      self
    ) {
      const token = tokens[idx];
      const info = token.info
        ? mdInstance.utils.unescapeAll(token.info).trim()
        : "";
      let langName = "";
      let highlighted;

      if (info) {
        langName = info.split(/\s+/g)[0];
        token.attrJoin(
          "class",
          options.langPrefix + langName.replace(/=$|=\d+$|=\+$|!$|=!/, "")
        );
        token.attrJoin("class", "hljs");
        token.attrJoin("class", "raw");
      }

      if (options.highlight) {
        highlighted =
          options.highlight(token.content, langName, "") ||
          mdInstance.utils.escapeHtml(token.content);
      } else {
        highlighted = mdInstance.utils.escapeHtml(token.content);
      }

      if (highlighted.indexOf("<pre") === 0) {
        return `${highlighted}\n`;
      }

      if (tokens[idx].map && tokens[idx].level === 0) {
        const startline = tokens[idx].map[0] + 1;
        const endline = tokens[idx].map[1];
        return `<pre class="part" data-startline="${startline}" data-endline="${endline}"><code${self.renderAttrs(token)}>${highlighted}</code></pre>\n`;
      }

      return `<pre><code${self.renderAttrs(token)}>${highlighted}</code></pre>\n`;
    };

    mdInstance.renderer.rules.code_block = function (
      tokens,
      idx,
      _options,
      _env,
      _self
    ) {
      if (tokens[idx].map && tokens[idx].level === 0) {
        const startline = tokens[idx].map[0] + 1;
        const endline = tokens[idx].map[1];
        return `<pre class="part" data-startline="${startline}" data-endline="${endline}"><code>${mdInstance.utils.escapeHtml(tokens[idx].content)}</code></pre>\n`;
      }
      return `<pre><code>${mdInstance.utils.escapeHtml(tokens[idx].content)}</code></pre>\n`;
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
  return md.render(content);
}
