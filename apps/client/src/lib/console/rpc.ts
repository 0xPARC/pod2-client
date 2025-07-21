import { invoke } from "@tauri-apps/api/core";
import type {
  ConsoleMessage,
  ConsoleState,
  ConsoleEvent
} from "../../components/console/types";

// Console RPC functions

export async function executeConsoleCommand(input: string): Promise<string> {
  return await invoke("console_execute_command", { input });
}

export async function getConsoleMessages(
  limit?: number
): Promise<ConsoleMessage[]> {
  return await invoke("console_get_messages", { limit });
}

export async function getConsoleState(): Promise<ConsoleState> {
  return await invoke("console_get_state");
}

export async function getCommandHistory(): Promise<string[]> {
  return await invoke("console_get_command_history");
}

export async function logConsoleEvent(event: ConsoleEvent): Promise<void> {
  return await invoke("console_log_event", { event });
}
