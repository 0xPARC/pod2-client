import type { Element, Root } from "hast";
import type { Plugin } from "unified";
import { visit } from "unist-util-visit";

interface MathElement extends Element {
  tagName: "mjx-container";
}

// Measure actual rendered height of math element
function getMathHeight(mathElement: Element): number {
  // Convert HAST element to HTML string
  const elementToHtml = (element: any): string => {
    if (element.type === "text") {
      return element.value;
    }
    if (element.type === "element") {
      const attrs = Object.entries(element.properties || {})
        .map(([key, value]) => `${key}="${value}"`)
        .join(" ");
      const children = (element.children || []).map(elementToHtml).join("");
      return `<${element.tagName}${attrs ? " " + attrs : ""}>${children}</${element.tagName}>`;
    }
    return "";
  };

  const htmlString = elementToHtml(mathElement);

  // Create a temporary DOM element to measure height
  if (typeof document !== "undefined") {
    const tempDiv = document.createElement("div");
    tempDiv.style.position = "absolute";
    tempDiv.style.visibility = "hidden";
    tempDiv.style.left = "-9999px";
    tempDiv.style.fontSize = "16px"; // Standard body font size
    tempDiv.style.lineHeight = "1.5"; // Standard line height
    tempDiv.innerHTML = htmlString;

    document.body.appendChild(tempDiv);
    const height = tempDiv.offsetHeight;
    document.body.removeChild(tempDiv);

    return height;
  }

  // Fallback for server-side rendering - return 0 to default to inline
  return 0;
}

const rehypeDisplayMath: Plugin<[], Root> = () => {
  return (tree: Root) => {
    visit(tree, "element", (node: Element) => {
      if (node.tagName === "p") {
        const mathChild = node.children.find(
          (child): child is MathElement =>
            child.type === "element" && child.tagName === "mjx-container"
        );

        if (mathChild) {
          // Measure the actual rendered height of the math element
          const height = getMathHeight(mathChild);

          // Standard line height (16px font * 1.5 line-height = 24px)
          const STANDARD_LINE_HEIGHT = 24;

          if (height > STANDARD_LINE_HEIGHT) {
            // Taller than one line, make it display math
            mathChild.properties = {
              ...mathChild.properties,
              style: "display: block; text-align: center; margin: 1em 0;"
            };
          }
        }
      }
    });
  };
};

export default rehypeDisplayMath;
