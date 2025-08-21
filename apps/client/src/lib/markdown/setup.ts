import MarkdownIt from "markdown-it";
import anchor from "markdown-it-anchor";
// @ts-expect-error no types
import taskLists from "@hackmd/markdown-it-task-lists";
import hljs from "markdown-it-highlightjs";
import markdownItMathjax from "markdown-it-mathjax3";

export type BaseMarkdownOptions = {
  enableLinkTargetBlank?: boolean;
  slugify?: (s: string) => string;
};

export function defaultSlugify(s: string): string {
  return s
    .toLowerCase()
    .replace(/[^a-z0-9]/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
}

export function createBaseMarkdownIt(
  opts: BaseMarkdownOptions = {}
): MarkdownIt {
  const { enableLinkTargetBlank = true, slugify = defaultSlugify } = opts;

  const md = new MarkdownIt({
    html: false,
    xhtmlOut: false,
    breaks: true,
    langPrefix: "language-",
    linkify: true,
    typographer: true
  })
    .use(anchor, {
      permalink: false,
      level: [1, 2, 3, 4, 5, 6],
      slugify
    })
    .use(hljs, {
      auto: false,
      code: true
    })
    .use(taskLists)
    .use(markdownItMathjax, {
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
    .enable(["table", "strikethrough"]);

  if (enableLinkTargetBlank) {
    md.renderer.rules.link_open = function (
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
  }

  return md;
}

// Utility to sanitize fenced codeblock language info to avoid highlight errors
export function sanitizeFenceInfo(info: string | undefined): string {
  if (!info) return "";
  let lang = info.trim();
  const spaceIndex = lang.indexOf(" ");
  if (spaceIndex > 0) lang = lang.substring(0, spaceIndex);
  if (lang.startsWith("{") || lang.includes("=") || lang.includes('"'))
    lang = "";
  if (lang.startsWith(".")) lang = lang.substring(1);
  return lang;
}
