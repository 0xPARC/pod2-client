// Block-level quoting utilities

export interface BlockSelection {
  indices: Set<number>;
  blocks: string[];
}

// Format selected blocks as markdown quotes
export function formatBlockQuotes(selectedBlocks: string[]): string {
  if (selectedBlocks.length === 0) return "";

  return (
    selectedBlocks
      .map((block) => {
        // Split block into lines and prefix each with '> '
        return block
          .split("\n")
          .map((line) => `> ${line}`)
          .join("\n");
      })
      .join("\n>\n") + "\n\n"
  );
}

// Get block elements from the DOM by their data-block-index attributes
export function getBlockElements(container: HTMLElement): HTMLElement[] {
  return Array.from(
    container.querySelectorAll("[data-block-index]")
  ) as HTMLElement[];
}

// Get block index from DOM element
export function getBlockIndex(element: HTMLElement): number {
  const index = element.getAttribute("data-block-index");
  return index ? parseInt(index, 10) : -1;
}

// Find block element by index
export function findBlockByIndex(
  container: HTMLElement,
  index: number
): HTMLElement | null {
  return container.querySelector(
    `[data-block-index="${index}"]`
  ) as HTMLElement | null;
}

// Get the vertical position information for a block element
export interface BlockPosition {
  index: number;
  top: number;
  height: number;
  element: HTMLElement;
}

export function getBlockPositions(container: HTMLElement): BlockPosition[] {
  const blockElements = getBlockElements(container);
  const containerRect = container.getBoundingClientRect();

  return blockElements
    .map((element) => {
      const rect = element.getBoundingClientRect();
      const index = getBlockIndex(element);

      return {
        index,
        top: rect.top - containerRect.top,
        height: rect.height,
        element
      };
    })
    .filter((pos) => pos.index >= 0)
    .sort((a, b) => a.index - b.index);
}

// Check if two block indices are adjacent
export function areBlocksAdjacent(indices: number[]): boolean {
  const sorted = [...indices].sort((a, b) => a - b);
  for (let i = 1; i < sorted.length; i++) {
    if (sorted[i] - sorted[i - 1] !== 1) {
      return false;
    }
  }
  return true;
}

// Group adjacent block indices into ranges
export function groupAdjacentBlocks(indices: number[]): number[][] {
  if (indices.length === 0) return [];

  const sorted = [...indices].sort((a, b) => a - b);
  const groups: number[][] = [];
  let currentGroup = [sorted[0]];

  for (let i = 1; i < sorted.length; i++) {
    if (sorted[i] - sorted[i - 1] === 1) {
      // Adjacent block, add to current group
      currentGroup.push(sorted[i]);
    } else {
      // Gap found, start new group
      groups.push(currentGroup);
      currentGroup = [sorted[i]];
    }
  }

  // Add the last group
  groups.push(currentGroup);

  return groups;
}
