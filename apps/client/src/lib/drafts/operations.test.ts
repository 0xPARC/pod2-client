/**
 * Tests for draft operations utility functions
 */

import { describe, it, expect, beforeEach, vi } from "vitest";
import {
  loadDraft,
  hasValidContent,
  buildDraftRequest,
  saveOnUnmount,
  deleteDraft,
  draftInfoToContent
} from "./operations";
import type { DraftContent } from "./types";
import type { DraftInfo } from "../documentApi";

// Mock the documentApi module
vi.mock("../documentApi", () => ({
  getDraft: vi.fn(),
  createDraft: vi.fn(),
  updateDraft: vi.fn(),
  deleteDraft: vi.fn()
}));

// Import the mocked functions
import {
  getDraft,
  createDraft,
  updateDraft,
  deleteDraft as deleteDraftApi
} from "../documentApi";

const mockGetDraft = vi.mocked(getDraft);
const mockCreateDraft = vi.mocked(createDraft);
const mockUpdateDraft = vi.mocked(updateDraft);
const mockDeleteDraft = vi.mocked(deleteDraftApi);

describe("Draft Operations", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("loadDraft", () => {
    it("should load and convert draft successfully", async () => {
      const mockDraftInfo: DraftInfo = {
        id: "test-id",
        title: "Test Title",
        content_type: "message",
        message: "Test message",
        tags: ["tag1", "tag2"],
        authors: ["author1"],
        reply_to: "post_123:456",
        created_at: "2024-01-01T00:00:00Z",
        updated_at: "2024-01-01T00:00:00Z"
      };

      mockGetDraft.mockResolvedValue(mockDraftInfo);

      const result = await loadDraft("test-id");

      expect(result).toEqual({
        title: "Test Title",
        message: "Test message",
        tags: ["tag1", "tag2"],
        authors: ["author1"],
        replyTo: "post_123:456"
      });

      expect(mockGetDraft).toHaveBeenCalledWith("test-id");
    });

    it("should return null when draft not found", async () => {
      mockGetDraft.mockResolvedValue(null);

      const result = await loadDraft("nonexistent");

      expect(result).toBeNull();
    });

    it("should handle missing message gracefully", async () => {
      const mockDraftInfo: DraftInfo = {
        id: "test-id",
        title: "Test Title",
        content_type: "message",
        tags: [],
        authors: [],
        created_at: "2024-01-01T00:00:00Z",
        updated_at: "2024-01-01T00:00:00Z"
      };

      mockGetDraft.mockResolvedValue(mockDraftInfo);

      const result = await loadDraft("test-id");

      expect(result?.message).toBe("");
    });

    it("should handle API errors gracefully", async () => {
      mockGetDraft.mockRejectedValue(new Error("API Error"));

      const result = await loadDraft("test-id");

      expect(result).toBeNull();
    });
  });

  describe("hasValidContent", () => {
    it("should return true for content with title", () => {
      const content: DraftContent = {
        title: "Test Title",
        message: "",
        tags: [],
        authors: []
      };

      expect(hasValidContent(content)).toBe(true);
    });

    it("should return true for content with message", () => {
      const content: DraftContent = {
        title: "",
        message: "Test message",
        tags: [],
        authors: []
      };

      expect(hasValidContent(content)).toBe(true);
    });

    it("should return true for content with tags", () => {
      const content: DraftContent = {
        title: "",
        message: "",
        tags: ["tag1"],
        authors: []
      };

      expect(hasValidContent(content)).toBe(true);
    });

    it("should return true for content with authors", () => {
      const content: DraftContent = {
        title: "",
        message: "",
        tags: [],
        authors: ["author1"]
      };

      expect(hasValidContent(content)).toBe(true);
    });

    it("should return false for empty content", () => {
      const content: DraftContent = {
        title: "",
        message: "",
        tags: [],
        authors: []
      };

      expect(hasValidContent(content)).toBe(false);
    });

    it("should return false for whitespace-only content", () => {
      const content: DraftContent = {
        title: "   ",
        message: "  \t  ",
        tags: [],
        authors: []
      };

      expect(hasValidContent(content)).toBe(false);
    });
  });

  describe("buildDraftRequest", () => {
    it("should build draft request correctly", () => {
      const content: DraftContent = {
        title: "Test Title",
        message: "Test message",
        tags: ["tag1", "tag2"],
        authors: ["author1"],
        replyTo: "post_123:456"
      };

      const result = buildDraftRequest(content);

      expect(result).toEqual({
        title: "Test Title",
        content_type: "message",
        message: "Test message",
        file_name: null,
        file_content: null,
        file_mime_type: null,
        url: null,
        tags: ["tag1", "tag2"],
        authors: ["author1"],
        reply_to: "post_123:456"
      });
    });

    it("should handle empty message as null", () => {
      const content: DraftContent = {
        title: "Test Title",
        message: "",
        tags: [],
        authors: []
      };

      const result = buildDraftRequest(content);

      expect(result.message).toBeNull();
    });

    it("should handle missing replyTo as null", () => {
      const content: DraftContent = {
        title: "Test Title",
        message: "Test message",
        tags: [],
        authors: []
      };

      const result = buildDraftRequest(content);

      expect(result.reply_to).toBeNull();
    });

    it("should trim title whitespace", () => {
      const content: DraftContent = {
        title: "  Test Title  ",
        message: "Test message",
        tags: [],
        authors: []
      };

      const result = buildDraftRequest(content);

      expect(result.title).toBe("Test Title");
    });
  });

  describe("saveOnUnmount", () => {
    it("should skip saving when no unsaved changes", async () => {
      const content: DraftContent = {
        title: "Test Title",
        message: "Test message",
        tags: [],
        authors: []
      };

      const result = await saveOnUnmount("test-id", content, false);

      expect(result).toEqual({ success: true });
      expect(mockCreateDraft).not.toHaveBeenCalled();
      expect(mockUpdateDraft).not.toHaveBeenCalled();
    });

    it("should skip saving when no valid content", async () => {
      const content: DraftContent = {
        title: "",
        message: "",
        tags: [],
        authors: []
      };

      const result = await saveOnUnmount("test-id", content, true);

      expect(result).toEqual({ success: true });
      expect(mockCreateDraft).not.toHaveBeenCalled();
      expect(mockUpdateDraft).not.toHaveBeenCalled();
    });

    it("should update existing draft", async () => {
      const content: DraftContent = {
        title: "Updated Title",
        message: "Updated message",
        tags: ["tag1"],
        authors: ["author1"]
      };

      mockUpdateDraft.mockResolvedValue(true);

      const result = await saveOnUnmount("existing-id", content, true);

      expect(result).toEqual({ success: true, draftId: "existing-id" });
      expect(mockUpdateDraft).toHaveBeenCalledWith("existing-id", {
        title: "Updated Title",
        content_type: "message",
        message: "Updated message",
        file_name: null,
        file_content: null,
        file_mime_type: null,
        url: null,
        tags: ["tag1"],
        authors: ["author1"],
        reply_to: null
      });
    });

    it("should create new draft when no draft ID", async () => {
      const content: DraftContent = {
        title: "New Title",
        message: "New message",
        tags: [],
        authors: []
      };

      mockCreateDraft.mockResolvedValue("new-draft-id");

      const result = await saveOnUnmount(undefined, content, true);

      expect(result).toEqual({ success: true, draftId: "new-draft-id" });
      expect(mockCreateDraft).toHaveBeenCalledWith({
        title: "New Title",
        content_type: "message",
        message: "New message",
        file_name: null,
        file_content: null,
        file_mime_type: null,
        url: null,
        tags: [],
        authors: [],
        reply_to: null
      });
    });

    it("should handle update failure", async () => {
      const content: DraftContent = {
        title: "Test Title",
        message: "Test message",
        tags: [],
        authors: []
      };

      mockUpdateDraft.mockResolvedValue(false);

      const result = await saveOnUnmount("test-id", content, true);

      expect(result).toEqual({
        success: false,
        error: "Failed to update draft"
      });
    });

    it("should handle API errors", async () => {
      const content: DraftContent = {
        title: "Test Title",
        message: "Test message",
        tags: [],
        authors: []
      };

      mockCreateDraft.mockRejectedValue(new Error("API Error"));

      const result = await saveOnUnmount(undefined, content, true);

      expect(result).toEqual({ success: false, error: "API Error" });
    });
  });

  describe("deleteDraft", () => {
    it("should delete draft successfully", async () => {
      mockDeleteDraft.mockResolvedValue(true);

      const result = await deleteDraft("test-id");

      expect(result).toBe(true);
      expect(mockDeleteDraft).toHaveBeenCalledWith("test-id");
    });

    it("should handle deletion failure", async () => {
      mockDeleteDraft.mockResolvedValue(false);

      const result = await deleteDraft("test-id");

      expect(result).toBe(false);
    });

    it("should handle API errors", async () => {
      mockDeleteDraft.mockRejectedValue(new Error("API Error"));

      const result = await deleteDraft("test-id");

      expect(result).toBe(false);
    });
  });

  describe("draftInfoToContent", () => {
    it("should convert DraftInfo to DraftContent", () => {
      const draftInfo: DraftInfo = {
        id: "test-id",
        title: "Test Title",
        content_type: "message",
        message: "Test message",
        tags: ["tag1", "tag2"],
        authors: ["author1"],
        reply_to: "post_123:456",
        created_at: "2024-01-01T00:00:00Z",
        updated_at: "2024-01-01T00:00:00Z"
      };

      const result = draftInfoToContent(draftInfo);

      expect(result).toEqual({
        title: "Test Title",
        message: "Test message",
        tags: ["tag1", "tag2"],
        authors: ["author1"],
        replyTo: "post_123:456"
      });
    });

    it("should handle missing message", () => {
      const draftInfo: DraftInfo = {
        id: "test-id",
        title: "Test Title",
        content_type: "message",
        tags: [],
        authors: [],
        created_at: "2024-01-01T00:00:00Z",
        updated_at: "2024-01-01T00:00:00Z"
      };

      const result = draftInfoToContent(draftInfo);

      expect(result.message).toBe("");
    });

    it("should handle missing replyTo", () => {
      const draftInfo: DraftInfo = {
        id: "test-id",
        title: "Test Title",
        content_type: "message",
        message: "Test message",
        tags: [],
        authors: [],
        created_at: "2024-01-01T00:00:00Z",
        updated_at: "2024-01-01T00:00:00Z"
      };

      const result = draftInfoToContent(draftInfo);

      expect(result.replyTo).toBeUndefined();
    });
  });
});
