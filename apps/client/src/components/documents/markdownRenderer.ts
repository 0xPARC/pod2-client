import MarkdownIt from "markdown-it";
import hljs from "markdown-it-highlightjs";
import markdownItMathjax from "markdown-it-mathjax3";
import { useMemo } from "react";

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
