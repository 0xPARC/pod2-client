/**
 * Core draft operations - testable utility functions
 */

import {
  createDraft,
  updateDraft,
  getDraft,
  deleteDraft as deleteDraftApi,
  type DraftRequest,
  type DraftInfo
} from "../documentApi";
import type { DraftContent, SaveOnUnmountResult } from "./types";

/**
 * Load draft content from a draft ID
 */
export async function loadDraft(draftId: string): Promise<DraftContent | null> {
  try {
    const draft = await getDraft(draftId);
    if (!draft) {
      return null;
    }

    return {
      title: draft.title,
      message: draft.message || "",
      tags: draft.tags,
      authors: draft.authors,
      replyTo: draft.reply_to
    };
  } catch (error) {
    console.error("Failed to load draft:", error);
    return null;
  }
}

/**
 * Check if draft content has meaningful data worth saving
 */
export function hasValidContent(content: DraftContent): boolean {
  return !!(
    content.title.trim() ||
    content.message.trim() ||
    content.tags.length > 0 ||
    content.authors.length > 0
  );
}

/**
 * Build a DraftRequest from DraftContent
 */
export function buildDraftRequest(content: DraftContent): DraftRequest {
  return {
    title: content.title.trim(),
    content_type: "message" as const,
    message: content.message || null,
    file_name: null,
    file_content: null,
    file_mime_type: null,
    url: null,
    tags: content.tags,
    authors: content.authors,
    reply_to: content.replyTo || null
  };
}

/**
 * Save draft on component unmount - preserves current behavior
 */
export async function saveOnUnmount(
  currentDraftId: string | undefined,
  content: DraftContent,
  hasUnsavedChanges: boolean
): Promise<SaveOnUnmountResult> {
  // Only save if we have unsaved changes and meaningful content
  if (!hasUnsavedChanges || !hasValidContent(content)) {
    return { success: true };
  }

  try {
    const draftRequest = buildDraftRequest(content);

    let resultDraftId: string;
    if (currentDraftId) {
      // Update existing draft
      const success = await updateDraft(currentDraftId, draftRequest);
      if (!success) {
        return { success: false, error: "Failed to update draft" };
      }
      resultDraftId = currentDraftId;
    } else {
      // Create new draft
      resultDraftId = await createDraft(draftRequest);
    }

    return { success: true, draftId: resultDraftId };
  } catch (error) {
    console.error("Failed to save draft on unmount:", error);
    return {
      success: false,
      error: error instanceof Error ? error.message : "Unknown error"
    };
  }
}

/**
 * Delete a draft
 */
export async function deleteDraft(draftId: string): Promise<boolean> {
  try {
    return await deleteDraftApi(draftId);
  } catch (error) {
    console.error("Failed to delete draft:", error);
    return false;
  }
}

/**
 * Convert DraftInfo to DraftContent
 */
export function draftInfoToContent(draft: DraftInfo): DraftContent {
  return {
    title: draft.title,
    message: draft.message || "",
    tags: draft.tags,
    authors: draft.authors,
    replyTo: draft.reply_to
  };
}
