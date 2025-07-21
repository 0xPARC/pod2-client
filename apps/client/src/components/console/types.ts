// Console types that mirror the Rust types

export interface ConsoleMessage {
  id: number;
  timestamp: string;
  message_type: MessageType;
  content: string;
  source: MessageSource;
  current_folder: string;
}

export type MessageType =
  | "Command"
  | "CommandResult"
  | "GuiEvent"
  | "SystemEvent"
  | "Error";

export type MessageSource = "Console" | "Gui" | "System";

export interface ConsoleState {
  current_folder: string;
  total_message_count: number;
  aliases_loaded: boolean;
  config_file_status: string;
}

export interface ConsoleEvent {
  timestamp: string;
  event_type: string;
  source: MessageSource;
  message: string;
  data?: any;
}

export type ValidationResult =
  | { type: "Valid" }
  | { type: "Invalid"; error: string; suggestion?: string };
