import MarkdownIt from "markdown-it";
import { beforeEach, describe, expect, it } from "vitest";
import { renderMarkdownWithBlocks } from "./markdownRenderer";

describe("renderMarkdownWithBlocks", () => {
  let md: MarkdownIt;

  beforeEach(() => {
    // Create a fresh markdown instance for each test
    md = new MarkdownIt({
      html: false,
      xhtmlOut: false,
      breaks: true,
      langPrefix: "language-",
      linkify: true,
      typographer: true
    });
  });

  it("should extract paragraph blocks correctly", () => {
    const content = `This is the first paragraph.

This is the second paragraph.

This is the third paragraph.`;

    const result = renderMarkdownWithBlocks(md, content);

    expect(result.blocks).toHaveLength(3);
    expect(result.blocks[0]).toBe("This is the first paragraph.");
    expect(result.blocks[1]).toBe("This is the second paragraph.");
    expect(result.blocks[2]).toBe("This is the third paragraph.");
  });

  it("should extract heading blocks correctly", () => {
    const content = `# Heading 1

## Heading 2

### Heading 3`;

    const result = renderMarkdownWithBlocks(md, content);

    expect(result.blocks).toHaveLength(3);
    expect(result.blocks[0]).toBe("# Heading 1");
    expect(result.blocks[1]).toBe("## Heading 2");
    expect(result.blocks[2]).toBe("### Heading 3");
  });

  it("should extract code blocks with fence markers", () => {
    const content = `Here's some code:

\`\`\`javascript
function hello() {
  console.log("Hello, world!");
}
\`\`\`

And some more text.`;

    const result = renderMarkdownWithBlocks(md, content);

    expect(result.blocks).toHaveLength(3);
    expect(result.blocks[0]).toBe("Here's some code:");
    expect(result.blocks[1]).toContain("```javascript");
    expect(result.blocks[1]).toContain("function hello()");
    expect(result.blocks[1]).toContain("```");
    expect(result.blocks[2]).toBe("And some more text.");
  });

  it("should extract list blocks as single units", () => {
    const content = `Here's a list:

- Item 1
- Item 2
- Item 3

And a numbered list:

1. First
2. Second
3. Third`;

    const result = renderMarkdownWithBlocks(md, content);

    expect(result.blocks).toHaveLength(4);
    expect(result.blocks[0]).toBe("Here's a list:");
    expect(result.blocks[1]).toContain("- Item 1");
    expect(result.blocks[1]).toContain("- Item 2");
    expect(result.blocks[1]).toContain("- Item 3");
    expect(result.blocks[2]).toBe("And a numbered list:");
    expect(result.blocks[3]).toContain("1. First");
    expect(result.blocks[3]).toContain("2. Second");
    expect(result.blocks[3]).toContain("3. Third");
  });

  it("should extract blockquotes correctly", () => {
    const content = `Normal text.

> This is a blockquote
> with multiple lines
> of quoted text.

More normal text.`;

    const result = renderMarkdownWithBlocks(md, content);

    expect(result.blocks).toHaveLength(3);
    expect(result.blocks[0]).toBe("Normal text.");
    expect(result.blocks[1]).toContain("> This is a blockquote");
    expect(result.blocks[1]).toContain("> with multiple lines");
    expect(result.blocks[1]).toContain("> of quoted text.");
    expect(result.blocks[2]).toBe("More normal text.");
  });

  it("should handle horizontal rules", () => {
    const content = `Text before.

---

Text after.`;

    const result = renderMarkdownWithBlocks(md, content);

    expect(result.blocks).toHaveLength(3);
    expect(result.blocks[0]).toBe("Text before.");
    expect(result.blocks[1]).toBe("---");
    expect(result.blocks[2]).toBe("Text after.");
  });

  it("should handle mixed content correctly", () => {
    const content = `# Main Title

This is an introduction paragraph.

## Section 1

Here's some content with a list:

- First item
- Second item

\`\`\`python
def greet(name):
    print(f"Hello, {name}!")
\`\`\`

> Important note: This is a blockquote.

---

Final paragraph.`;

    const result = renderMarkdownWithBlocks(md, content);

    expect(result.blocks).toHaveLength(9);
    expect(result.blocks[0]).toBe("# Main Title");
    expect(result.blocks[1]).toBe("This is an introduction paragraph.");
    expect(result.blocks[2]).toBe("## Section 1");
    expect(result.blocks[3]).toBe("Here's some content with a list:");
    expect(result.blocks[4]).toContain("- First item");
    expect(result.blocks[4]).toContain("- Second item");
    expect(result.blocks[5]).toContain("```python");
    expect(result.blocks[5]).toContain("def greet(name):");
    expect(result.blocks[6]).toContain(
      "> Important note: This is a blockquote."
    );
    expect(result.blocks[7]).toBe("---");
    expect(result.blocks[8]).toBe("Final paragraph.");
  });

  it("should handle empty content", () => {
    const result = renderMarkdownWithBlocks(md, "");

    expect(result.blocks).toHaveLength(0);
    expect(result.html).toBe("");
  });

  it("should handle content with only whitespace", () => {
    const result = renderMarkdownWithBlocks(md, "   \n\n   ");

    expect(result.blocks).toHaveLength(0);
    expect(result.html).toBe("");
  });

  it("should preserve math blocks", () => {
    const content = `Here's an equation:

$$
E = mc^2
$$

And inline math: $a^2 + b^2 = c^2$`;

    const result = renderMarkdownWithBlocks(md, content);

    // Note: Math block handling depends on markdown-it-mathjax3 plugin
    // For now, we just verify blocks are extracted
    expect(result.blocks.length).toBeGreaterThan(0);
    expect(result.blocks[0]).toBe("Here's an equation:");
  });

  it("should produce valid HTML output alongside block extraction", () => {
    const content = `# Title

Paragraph one.

Paragraph two.

- List item`;

    const result = renderMarkdownWithBlocks(md, content);

    // Verify we extracted blocks
    expect(result.blocks.length).toBe(4);
    expect(result.blocks[0]).toBe("# Title");
    expect(result.blocks[1]).toBe("Paragraph one.");
    expect(result.blocks[2]).toBe("Paragraph two.");
    expect(result.blocks[3]).toBe("- List item");

    // Verify HTML was generated (basic check)
    expect(result.html).toContain("<h1");
    expect(result.html).toContain("<p>");
    expect(result.html).toContain("<ul>");
  });
});
