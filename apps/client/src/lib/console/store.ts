import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ConsoleMessage,
  ConsoleState
} from "../../components/console/types";
import {
  executeConsoleCommand,
  getConsoleMessages,
  getConsoleState,
  getCommandHistory
} from "./rpc";

// Global singleton to prevent duplicate listeners - resistant to React Strict Mode
let globalInitializationPromise: Promise<void> | null = null;
let globalConsoleUpdatedUnlisten: UnlistenFn | null = null;
let globalConsoleClearedUnlisten: UnlistenFn | null = null;

// This function will only ever run once, even with React Strict Mode
async function initializeGlobalListeners(set: any): Promise<void> {
  if (globalInitializationPromise) {
    // If initialization is already in progress or completed, wait for it
    return globalInitializationPromise;
  }

  console.log(
    "Frontend: Initializing global console listeners (React Strict Mode safe)"
  );

  globalInitializationPromise = (async () => {
    // Clean up any existing listeners first
    if (globalConsoleUpdatedUnlisten) {
      globalConsoleUpdatedUnlisten();
      globalConsoleUpdatedUnlisten = null;
    }
    if (globalConsoleClearedUnlisten) {
      globalConsoleClearedUnlisten();
      globalConsoleClearedUnlisten = null;
    }

    // Set up new listeners
    globalConsoleUpdatedUnlisten = await listen("console-updated", (event) => {
      const message = event.payload as ConsoleMessage;
      // console.log("Frontend: Received console-updated event:", message); // Debug: can re-enable if needed
      set((state: any) => ({
        messages: [...state.messages, message]
      }));
    });

    globalConsoleClearedUnlisten = await listen("console-cleared", () => {
      set({ messages: [] });
    });
  })();

  return globalInitializationPromise;
}

interface ConsoleStore {
  // State
  messages: ConsoleMessage[];
  state: ConsoleState | null;
  commandHistory: string[];
  historyIndex: number;
  inputValue: string;
  isLoading: boolean;
  error: string | null;
  isInitialized: boolean;

  // Actions
  initialize: () => Promise<void>;
  cleanup: () => Promise<void>;
  executeCommand: (input: string) => Promise<void>;
  loadMessages: () => Promise<void>;
  loadState: () => Promise<void>;
  loadCommandHistory: () => Promise<void>;
  setInputValue: (value: string) => void;
  navigateHistory: (direction: "up" | "down") => void;
  clearError: () => void;
}

export const useConsoleStore = create<ConsoleStore>((set, get) => ({
  // Initial state
  messages: [],
  state: null,
  commandHistory: [],
  historyIndex: -1,
  inputValue: "",
  isLoading: false,
  error: null,
  isInitialized: false,

  initialize: async () => {
    const { isInitialized } = get();

    // Prevent multiple initializations
    if (isInitialized) {
      return;
    }

    try {
      set({ isLoading: true, error: null });

      // Load initial state
      await Promise.all([
        get().loadMessages(),
        get().loadState(),
        get().loadCommandHistory()
      ]);

      // Initialize global listeners (React Strict Mode safe)
      await initializeGlobalListeners(set);

      set({ isLoading: false, isInitialized: true });
    } catch (error) {
      set({
        error: `Failed to initialize console: ${error}`,
        isLoading: false
      });
    }
  },

  cleanup: async () => {
    // Only clean up global listeners when explicitly requested (e.g., app shutdown)
    // Don't clean up on component unmount to prevent issues with multiple components
    console.log("Frontend: Cleanup called (but preserving global listeners)");
    set({ isInitialized: false });
  },

  executeCommand: async (input: string) => {
    try {
      set({ error: null });
      const trimmedInput = input.trim();

      if (!trimmedInput) return;

      // console.log(`Frontend: Executing command '${trimmedInput}'`); // Debug: can re-enable if needed

      // Clear input and reset history navigation immediately
      set({
        inputValue: "",
        historyIndex: -1
      });

      // Execute the command (this can take time, but input is already cleared)
      await executeConsoleCommand(trimmedInput);

      // Refresh command history
      await get().loadCommandHistory();
    } catch (error) {
      set({ error: `Command failed: ${error}` });
    }
  },

  loadMessages: async () => {
    try {
      const messages = await getConsoleMessages(100);
      set({ messages });
    } catch (error) {
      set({ error: `Failed to load messages: ${error}` });
    }
  },

  loadState: async () => {
    try {
      const state = await getConsoleState();
      set({ state });
    } catch (error) {
      set({ error: `Failed to load console state: ${error}` });
    }
  },

  loadCommandHistory: async () => {
    try {
      const history = await getCommandHistory();
      set({ commandHistory: history });
    } catch (error) {
      console.warn("Failed to load command history:", error);
    }
  },

  setInputValue: (value: string) => {
    set({ inputValue: value, historyIndex: -1 });
  },

  navigateHistory: (direction: "up" | "down") => {
    const { commandHistory, historyIndex } = get();

    if (commandHistory.length === 0) return;

    let newIndex = historyIndex;

    if (direction === "up") {
      newIndex =
        historyIndex < commandHistory.length - 1
          ? historyIndex + 1
          : historyIndex;
    } else {
      newIndex = historyIndex > -1 ? historyIndex - 1 : -1;
    }

    const newValue =
      newIndex === -1
        ? ""
        : commandHistory[commandHistory.length - 1 - newIndex];

    set({
      historyIndex: newIndex,
      inputValue: newValue
    });
  },

  clearError: () => {
    set({ error: null });
  }
}));
