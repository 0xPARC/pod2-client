import { useEffect, RefObject } from "react";

export interface UseTextSelectionOptions {
  contentRef: RefObject<HTMLElement | null>;
  enabled?: boolean;
  onQuoteText?: (text: string) => Promise<void>;
  minSelectionLength?: number;
}

export const useTextSelection = ({
  contentRef,
  enabled = true,
  onQuoteText,
  minSelectionLength = 3
}: UseTextSelectionOptions) => {
  useEffect(() => {
    if (!enabled || !onQuoteText) return;

    let quoteButtonElement: HTMLElement | null = null;

    const showQuoteButton = (selection: Selection, range: Range) => {
      // Remove any existing button
      if (quoteButtonElement) {
        quoteButtonElement.remove();
        quoteButtonElement = null;
      }

      const text = range.toString().trim();
      if (text.length < minSelectionLength) return;

      // Check if selection is within our content area
      if (
        !contentRef.current ||
        !contentRef.current.contains(range.commonAncestorContainer)
      ) {
        return;
      }

      // Create button element directly in DOM (no React state)
      const rect = range.getBoundingClientRect();
      const button = document.createElement("button");
      button.innerHTML = `
        <svg class="h-3 w-3 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"/>
        </svg>
        Quote & Reply
      `;

      button.className =
        "fixed z-50 px-2 py-1 text-xs bg-blue-600 hover:bg-blue-700 text-white shadow-lg border border-blue-500 rounded select-none";
      button.style.left = `${rect.left + rect.width / 2 - 50}px`; // Center horizontally on selection
      button.style.top = `${rect.top - 35}px`; // Position above selection
      button.style.pointerEvents = "auto";

      button.onclick = async (e) => {
        e.preventDefault();
        e.stopPropagation();

        // Store the selected text
        const selectedText = text;

        // Remove button
        button.remove();
        quoteButtonElement = null;

        // Clear selection to avoid further interference
        selection.removeAllRanges();

        // Handle quote selection with the stored text
        await onQuoteText(selectedText);
      };

      document.body.appendChild(button);
      quoteButtonElement = button;
    };

    const hideQuoteButton = () => {
      if (quoteButtonElement) {
        quoteButtonElement.remove();
        quoteButtonElement = null;
      }
    };

    const handleMouseUp = () => {
      setTimeout(() => {
        const selection = window.getSelection();
        if (
          selection &&
          selection.rangeCount > 0 &&
          selection.toString().trim().length >= minSelectionLength
        ) {
          const range = selection.getRangeAt(0);
          showQuoteButton(selection, range);
        } else {
          hideQuoteButton();
        }
      }, 100);
    };

    const handleClick = (e: MouseEvent) => {
      // Don't hide if clicking on our button
      if (
        (e.target as Element)
          ?.closest("button")
          ?.innerHTML?.includes("Quote & Reply")
      ) {
        return;
      }

      setTimeout(() => {
        const selection = window.getSelection();
        if (!selection || selection.toString().trim().length === 0) {
          hideQuoteButton();
        }
      }, 50);
    };

    document.addEventListener("mouseup", handleMouseUp);
    document.addEventListener("click", handleClick);

    return () => {
      hideQuoteButton();
      document.removeEventListener("mouseup", handleMouseUp);
      document.removeEventListener("click", handleClick);
    };
  }, [enabled, onQuoteText, contentRef, minSelectionLength]);
};
