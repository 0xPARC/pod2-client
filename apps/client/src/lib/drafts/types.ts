/**
 * Types for draft management system
 */

/**
 * Core draft content interface - simplified view of what the form manages
 */
export interface DraftContent {
  title: string;
  message: string;
  tags: string[];
  authors: string[];
  replyTo?: string;
}

/**
 * Options for the useDraftAutoSave hook
 */
export interface UseDraftAutoSaveOptions {
  editingDraftId?: string;
}

/**
 * Return type for the useDraftAutoSave hook
 */
export interface UseDraftAutoSaveReturn {
  // State
  currentDraftId?: string;
  hasUnsavedChanges: boolean;
  isLoading: boolean;

  // Actions
  markContentChanged: () => void; // Call when user modifies content
  discardDraft: () => Promise<boolean>;
  markSaved: () => void; // Call after successful publish
  registerContentGetter: (getter: () => DraftContent) => void; // Register content getter for save-on-unmount

  // Data
  initialContent?: DraftContent; // Loaded draft content
}

/**
 * Result of save-on-unmount operation
 */
export interface SaveOnUnmountResult {
  success: boolean;
  draftId?: string;
  error?: string;
}
