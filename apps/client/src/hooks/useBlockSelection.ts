import { useState, useCallback } from "react";
import { formatBlockQuotes, groupAdjacentBlocks } from "../lib/blockUtils";

export interface UseBlockSelectionOptions {
  blocks: string[];
  onQuoteText?: (text: string) => Promise<void>;
}

export interface UseBlockSelectionReturn {
  selectedBlocks: Set<number>;
  toggleBlock: (blockIndex: number, shiftKey?: boolean) => void;
  clearSelection: () => void;
  handleQuoteSelected: () => Promise<void>;
  isBlockSelected: (blockIndex: number) => boolean;
}

export function useBlockSelection({
  blocks,
  onQuoteText
}: UseBlockSelectionOptions): UseBlockSelectionReturn {
  const [selectedBlocks, setSelectedBlocks] = useState<Set<number>>(new Set());
  const [lastSelectedIndex, setLastSelectedIndex] = useState<number | null>(
    null
  );

  const toggleBlock = useCallback(
    (blockIndex: number, shiftKey = false) => {
      if (blockIndex < 0 || blockIndex >= blocks.length) return;

      setSelectedBlocks((prev) => {
        const newSelection = new Set(prev);

        if (shiftKey && lastSelectedIndex !== null) {
          // Range selection - select all blocks between last selected and current
          const start = Math.min(lastSelectedIndex, blockIndex);
          const end = Math.max(lastSelectedIndex, blockIndex);

          for (let i = start; i <= end; i++) {
            newSelection.add(i);
          }
        } else {
          // Single block toggle
          if (newSelection.has(blockIndex)) {
            newSelection.delete(blockIndex);
          } else {
            newSelection.add(blockIndex);
          }
          setLastSelectedIndex(blockIndex);
        }

        return newSelection;
      });
    },
    [blocks.length, lastSelectedIndex]
  );

  const clearSelection = useCallback(() => {
    setSelectedBlocks(new Set());
    setLastSelectedIndex(null);
  }, []);

  const isBlockSelected = useCallback(
    (blockIndex: number) => {
      return selectedBlocks.has(blockIndex);
    },
    [selectedBlocks]
  );

  const handleQuoteSelected = useCallback(async () => {
    if (selectedBlocks.size === 0 || !onQuoteText) return;

    // Get selected blocks in order
    const sortedIndices = Array.from(selectedBlocks).sort((a, b) => a - b);

    // Group adjacent blocks for better quote formatting
    const groups = groupAdjacentBlocks(sortedIndices);
    const groupedBlockTexts = groups.map((group) =>
      group.map((index) => blocks[index])
    );

    // Format as quotes
    let quotedText = "";

    if (groups.length === 1) {
      // Single group of blocks
      quotedText = formatBlockQuotes(groupedBlockTexts[0]);
    } else {
      // Multiple groups - separate them
      quotedText = groupedBlockTexts
        .map((groupBlocks) => formatBlockQuotes(groupBlocks))
        .join("\n");
    }

    try {
      await onQuoteText(quotedText);
      clearSelection();
    } catch (error) {
      console.error("Failed to quote blocks:", error);
    }
  }, [selectedBlocks, blocks, onQuoteText, clearSelection]);

  return {
    selectedBlocks,
    toggleBlock,
    clearSelection,
    handleQuoteSelected,
    isBlockSelected
  };
}
