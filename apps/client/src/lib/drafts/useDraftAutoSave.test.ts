/**
 * Tests for useDraftAutoSave hook
 */

import { describe, it, expect, beforeEach, vi, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { useDraftAutoSave } from "./useDraftAutoSave";
import type { DraftContent } from "./types";

// Mock the operations module
vi.mock("./operations", () => ({
  loadDraft: vi.fn(),
  saveOnUnmount: vi.fn(),
  deleteDraft: vi.fn()
}));

// Mock sonner toast
vi.mock("sonner", () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn()
  }
}));

// Import the mocked functions
import { loadDraft, saveOnUnmount, deleteDraft } from "./operations";
import { toast } from "sonner";

const mockLoadDraft = vi.mocked(loadDraft);
const mockSaveOnUnmount = vi.mocked(saveOnUnmount);
const mockDeleteDraft = vi.mocked(deleteDraft);
const mockToast = vi.mocked(toast);

describe("useDraftAutoSave", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.clearAllTimers();
  });

  describe("Initial state", () => {
    it("should initialize with correct default state", () => {
      const { result } = renderHook(() =>
        useDraftAutoSave({ editingDraftId: undefined })
      );

      expect(result.current.currentDraftId).toBeUndefined();
      expect(result.current.hasUnsavedChanges).toBe(false);
      expect(result.current.isLoading).toBe(false);
      expect(result.current.initialContent).toBeUndefined();
    });

    it("should set currentDraftId from editingDraftId", () => {
      const { result } = renderHook(() =>
        useDraftAutoSave({ editingDraftId: "test-draft-id" })
      );

      expect(result.current.currentDraftId).toBe("test-draft-id");
    });
  });

  describe("Draft loading", () => {
    it("should load existing draft on mount", async () => {
      const mockContent: DraftContent = {
        title: "Test Title",
        message: "Test message",
        tags: ["tag1"],
        authors: ["author1"],
        replyTo: "post_123:456"
      };

      mockLoadDraft.mockResolvedValue(mockContent);

      const { result } = renderHook(() =>
        useDraftAutoSave({ editingDraftId: "test-draft-id" })
      );

      // Initially loading
      expect(result.current.isLoading).toBe(true);

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      expect(result.current.initialContent).toEqual(mockContent);
      expect(result.current.hasUnsavedChanges).toBe(false);
      expect(mockLoadDraft).toHaveBeenCalledWith("test-draft-id");
    });

    it("should handle draft loading failure gracefully", async () => {
      mockLoadDraft.mockResolvedValue(null);

      const { result } = renderHook(() =>
        useDraftAutoSave({ editingDraftId: "nonexistent-draft" })
      );

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      expect(result.current.initialContent).toBeUndefined();
    });

    it("should skip loading when no editingDraftId", () => {
      const { result } = renderHook(() =>
        useDraftAutoSave({ editingDraftId: undefined })
      );

      expect(result.current.isLoading).toBe(false);
      expect(mockLoadDraft).not.toHaveBeenCalled();
    });
  });

  describe("Content change tracking", () => {
    it("should mark content as changed", () => {
      const { result } = renderHook(() =>
        useDraftAutoSave({ editingDraftId: undefined })
      );

      expect(result.current.hasUnsavedChanges).toBe(false);

      act(() => {
        result.current.markContentChanged();
      });

      expect(result.current.hasUnsavedChanges).toBe(true);
    });

    it("should mark content as saved", () => {
      const { result } = renderHook(() =>
        useDraftAutoSave({ editingDraftId: undefined })
      );

      // First mark as changed
      act(() => {
        result.current.markContentChanged();
      });

      expect(result.current.hasUnsavedChanges).toBe(true);

      // Then mark as saved
      act(() => {
        result.current.markSaved();
      });

      expect(result.current.hasUnsavedChanges).toBe(false);
    });
  });

  describe("Draft deletion", () => {
    it("should discard draft successfully", async () => {
      mockDeleteDraft.mockResolvedValue(true);

      const { result } = renderHook(() =>
        useDraftAutoSave({ editingDraftId: "test-draft-id" })
      );

      let discardResult: boolean;

      await act(async () => {
        discardResult = await result.current.discardDraft();
      });

      expect(discardResult!).toBe(true);
      expect(result.current.currentDraftId).toBeUndefined();
      expect(result.current.hasUnsavedChanges).toBe(false);
      expect(mockDeleteDraft).toHaveBeenCalledWith("test-draft-id");
      expect(mockToast.success).toHaveBeenCalledWith("Draft discarded");
    });

    it("should handle draft deletion failure", async () => {
      mockDeleteDraft.mockResolvedValue(false);

      const { result } = renderHook(() =>
        useDraftAutoSave({ editingDraftId: "test-draft-id" })
      );

      let discardResult: boolean;

      await act(async () => {
        discardResult = await result.current.discardDraft();
      });

      expect(discardResult!).toBe(false);
      expect(result.current.currentDraftId).toBe("test-draft-id"); // Should remain unchanged
      expect(mockToast.error).toHaveBeenCalledWith("Failed to discard draft");
    });

    it("should discard draft when no draft ID exists", async () => {
      const { result } = renderHook(() =>
        useDraftAutoSave({ editingDraftId: undefined })
      );

      let discardResult: boolean;

      await act(async () => {
        discardResult = await result.current.discardDraft();
      });

      expect(discardResult!).toBe(true);
      expect(result.current.hasUnsavedChanges).toBe(false);
      expect(mockDeleteDraft).not.toHaveBeenCalled();
      expect(mockToast.success).toHaveBeenCalledWith("Draft discarded");
    });

    it("should handle API errors during deletion", async () => {
      mockDeleteDraft.mockRejectedValue(new Error("API Error"));

      const { result } = renderHook(() =>
        useDraftAutoSave({ editingDraftId: "test-draft-id" })
      );

      let discardResult: boolean;

      await act(async () => {
        discardResult = await result.current.discardDraft();
      });

      expect(discardResult!).toBe(false);
      expect(mockToast.error).toHaveBeenCalledWith("Failed to discard draft");
    });
  });

  describe("Content getter registration", () => {
    it("should register content getter function", () => {
      const { result } = renderHook(() =>
        useDraftAutoSave({ editingDraftId: undefined })
      );

      const mockContentGetter = vi.fn(() => ({
        title: "Test",
        message: "Test",
        tags: [],
        authors: []
      }));

      act(() => {
        result.current.registerContentGetter(mockContentGetter);
      });

      // The hook should store the getter function internally
      // We can't directly test the ref, but we can test that it doesn't crash
      expect(() =>
        result.current.registerContentGetter(mockContentGetter)
      ).not.toThrow();
    });
  });

  describe("Save on unmount behavior", () => {
    it("should save content on unmount when changes exist", async () => {
      mockSaveOnUnmount.mockResolvedValue({
        success: true,
        draftId: "new-draft-id"
      });

      const mockContentGetter = vi.fn(() => ({
        title: "Test Title",
        message: "Test message",
        tags: ["tag1"],
        authors: []
      }));

      const { result, unmount } = renderHook(() =>
        useDraftAutoSave({ editingDraftId: undefined })
      );

      // Register content getter and mark as changed
      act(() => {
        result.current.registerContentGetter(mockContentGetter);
        result.current.markContentChanged();
      });

      // Unmount should trigger save
      unmount();

      // Wait for async save operation
      await waitFor(() => {
        expect(mockSaveOnUnmount).toHaveBeenCalled();
      });

      expect(mockContentGetter).toHaveBeenCalled();
    });

    it("should not save on unmount when no changes", () => {
      const mockContentGetter = vi.fn(() => ({
        title: "Test Title",
        message: "Test message",
        tags: [],
        authors: []
      }));

      const { result, unmount } = renderHook(() =>
        useDraftAutoSave({ editingDraftId: undefined })
      );

      // Register content getter but don't mark as changed
      act(() => {
        result.current.registerContentGetter(mockContentGetter);
      });

      unmount();

      // Should not attempt to save
      expect(mockSaveOnUnmount).not.toHaveBeenCalled();
    });

    it("should not save on unmount when no content getter registered", () => {
      const { result, unmount } = renderHook(() =>
        useDraftAutoSave({ editingDraftId: undefined })
      );

      // Mark as changed but don't register content getter
      act(() => {
        result.current.markContentChanged();
      });

      unmount();

      // Should not attempt to save without content getter
      expect(mockSaveOnUnmount).not.toHaveBeenCalled();
    });
  });

  describe("Integration scenarios", () => {
    it("should handle complete draft lifecycle", async () => {
      const initialContent: DraftContent = {
        title: "Initial Title",
        message: "Initial message",
        tags: [],
        authors: []
      };

      mockLoadDraft.mockResolvedValue(initialContent);
      mockDeleteDraft.mockResolvedValue(true);

      const { result } = renderHook(() =>
        useDraftAutoSave({ editingDraftId: "test-draft-id" })
      );

      // Wait for initial load
      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      // Verify initial state
      expect(result.current.initialContent).toEqual(initialContent);
      expect(result.current.hasUnsavedChanges).toBe(false);

      // Make changes
      act(() => {
        result.current.markContentChanged();
      });

      expect(result.current.hasUnsavedChanges).toBe(true);

      // Discard draft
      let discardResult: boolean;
      await act(async () => {
        discardResult = await result.current.discardDraft();
      });

      expect(discardResult!).toBe(true);
      expect(result.current.hasUnsavedChanges).toBe(false);
      expect(result.current.currentDraftId).toBeUndefined();
    });
  });
});
