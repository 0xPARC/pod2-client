/**
 * Draft management system - public exports
 */

export type {
  DraftContent,
  UseDraftAutoSaveOptions,
  UseDraftAutoSaveReturn,
  SaveOnUnmountResult
} from "./types";

export {
  loadDraft,
  hasValidContent,
  buildDraftRequest,
  saveOnUnmount,
  deleteDraft,
  draftInfoToContent
} from "./operations";

export { useDraftAutoSave } from "./useDraftAutoSave";
