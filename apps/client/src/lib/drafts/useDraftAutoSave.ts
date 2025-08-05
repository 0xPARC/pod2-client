/**
 * Custom hook for draft auto-saving functionality
 * Preserves save-on-unmount behavior while making it testable
 */

import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import type {
  DraftContent,
  UseDraftAutoSaveOptions,
  UseDraftAutoSaveReturn
} from "./types";
import { loadDraft, saveOnUnmount, deleteDraft } from "./operations";

export function useDraftAutoSave(
  options: UseDraftAutoSaveOptions
): UseDraftAutoSaveReturn {
  const { editingDraftId } = options;

  // State
  const [currentDraftId, setCurrentDraftId] = useState<string | undefined>(
    editingDraftId
  );
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [initialContent, setInitialContent] = useState<
    DraftContent | undefined
  >();

  // Refs to capture current state for cleanup
  const hasUnsavedChangesRef = useRef(hasUnsavedChanges);
  const currentDraftIdRef = useRef(currentDraftId);

  // Update refs when state changes
  useEffect(() => {
    hasUnsavedChangesRef.current = hasUnsavedChanges;
  }, [hasUnsavedChanges]);

  useEffect(() => {
    currentDraftIdRef.current = currentDraftId;
  }, [currentDraftId]);

  // Load existing draft on mount
  useEffect(() => {
    const loadExistingDraft = async () => {
      if (editingDraftId) {
        setIsLoading(true);
        try {
          const content = await loadDraft(editingDraftId);
          if (content) {
            setInitialContent(content);
            setHasUnsavedChanges(false); // Draft is freshly loaded, no unsaved changes
          }
        } catch (error) {
          console.error("Failed to load draft:", error);
        } finally {
          setIsLoading(false);
        }
      }
    };

    loadExistingDraft();
  }, [editingDraftId]);

  // Ref to store the content getter function from the component
  const contentGetterRef = useRef<(() => DraftContent) | null>(null);

  // Register content getter function
  const registerContentGetter = (getter: () => DraftContent) => {
    contentGetterRef.current = getter;
  };

  // Save-on-unmount cleanup effect - preserves current behavior
  useEffect(() => {
    return () => {
      // Use ref values to get current state, not stale closure values
      const currentHasUnsavedChanges = hasUnsavedChangesRef.current;
      const currentDraftIdValue = currentDraftIdRef.current;
      const contentGetter = contentGetterRef.current;

      // Only save if we have unsaved changes and a way to get content
      if (currentHasUnsavedChanges && contentGetter) {
        const currentContent = contentGetter();

        // Fire and forget save operation
        saveOnUnmount(
          currentDraftIdValue,
          currentContent,
          currentHasUnsavedChanges
        )
          .then((result) => {
            if (!result.success) {
              console.error(
                "[DraftAutoSave] Failed to save on unmount:",
                result.error
              );
            }
          })
          .catch((error) => {
            console.error("[DraftAutoSave] Error saving on unmount:", error);
          });
      }
    };
  }, []); // Empty deps - only runs on mount/unmount

  // Actions
  const markContentChanged = () => {
    setHasUnsavedChanges(true);
  };

  const discardDraft = async (): Promise<boolean> => {
    try {
      const draftIdToDelete = currentDraftId || editingDraftId;

      if (draftIdToDelete) {
        const success = await deleteDraft(draftIdToDelete);

        if (success) {
          toast.success("Draft discarded");
          setCurrentDraftId(undefined);
          setHasUnsavedChanges(false);
          return true;
        } else {
          toast.error("Failed to discard draft");
          return false;
        }
      } else {
        // No draft to delete, just mark as saved
        toast.success("Draft discarded");
        setHasUnsavedChanges(false);
        return true;
      }
    } catch (error) {
      console.error("Failed to delete draft:", error);
      toast.error("Failed to discard draft");
      return false;
    }
  };

  const markSaved = () => {
    setHasUnsavedChanges(false);
  };

  return {
    // State
    currentDraftId,
    hasUnsavedChanges,
    isLoading,

    // Actions
    markContentChanged,
    discardDraft,
    markSaved,
    registerContentGetter,

    // Data
    initialContent
  };
}
