/**
 * Vitest setup file
 * This file is executed before each test file runs
 */

// TODO: Fix jest-dom import issue - temporarily commented out
// import "@testing-library/jest-dom";
import { vi } from "vitest";

// Type declarations for global properties
declare global {
  var testUtils: {
    mockInvoke: ReturnType<typeof vi.fn>;
    mockOnOpenUrl: ReturnType<typeof vi.fn>;
  };
  var restoreConsole: () => void;
}

// Mock Tauri APIs for testing
global.window = global.window || {};

// Mock Tauri core invoke function
const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke
}));

// Mock Tauri deep-link plugin
const mockOnOpenUrl = vi.fn();
vi.mock("@tauri-apps/plugin-deep-link", () => ({
  onOpenUrl: mockOnOpenUrl
}));

// Mock Tauri window APIs
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    maximize: vi.fn()
  })
}));

// Export mocks for use in tests
export { mockInvoke, mockOnOpenUrl };

// Global test utilities
global.testUtils = {
  mockInvoke,
  mockOnOpenUrl
};

// Console setup - suppress console.log in tests unless specifically needed
const originalConsole = { ...console };
global.console = {
  ...console,
  log: vi.fn(),
  warn: vi.fn(),
  error: vi.fn(),
  info: vi.fn()
};

// Restore console for debugging if needed
global.restoreConsole = () => {
  global.console = originalConsole;
};
