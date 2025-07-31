import type { Element, Root, Text } from "hast";
import type { Plugin } from "unified";
import { visit } from "unist-util-visit";

interface MathElement extends Element {
  tagName: "mjx-container";
}

const rehypeDisplayMath: Plugin<[], Root> = () => {
  return (tree: Root) => {
    visit(tree, "element", (node: Element) => {
      console.log("node: ", node);
      if (node.tagName === "p") {
        const mathChild = node.children.find(
          (child): child is MathElement =>
            child.type === "element" && child.tagName === "mjx-container"
        );

        if (mathChild) {
          // Check if there are other meaningful children
          const otherChildren = node.children.filter((child) => {
            if (child === mathChild) return false;
            if (child.type === "text") {
              const textNode = child as Text;
              return textNode.value.trim() !== "";
            }
            return true;
          });

          if (otherChildren.length === 0) {
            // No other content, make it display math
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
