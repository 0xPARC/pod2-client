import type { ConsoleMessage, MessageType, MessageSource } from "./types";

interface ConsoleMessageProps {
  message: ConsoleMessage;
}

export function ConsoleMessage({ message }: ConsoleMessageProps) {
  const formatTimestamp = (timestamp: string) => {
    return new Date(timestamp).toLocaleTimeString([], {
      hour12: false,
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit"
    });
  };

  const getMessageTypeStyle = (type: MessageType): string => {
    switch (type) {
      case "Command":
        return "text-blue-600 dark:text-blue-400";
      case "CommandResult":
        return "text-green-600 dark:text-green-400";
      case "GuiEvent":
        return "text-purple-600 dark:text-purple-400";
      case "SystemEvent":
        return "text-orange-600 dark:text-orange-400";
      case "Error":
        return "text-red-600 dark:text-red-400";
      default:
        return "text-gray-600 dark:text-gray-400";
    }
  };

  const getSourceIcon = (source: MessageSource): string => {
    switch (source) {
      case "Console":
        return ">";
      case "Gui":
        return "🖱️";
      case "System":
        return "⚙️";
      default:
        return "";
    }
  };

  const shouldShowPrompt = message.message_type === "Command";
  const shouldShowFolder =
    shouldShowPrompt && message.current_folder !== "default";

  return (
    <div className="flex items-baseline gap-2 px-3 font-mono text-sm">
      {/* Timestamp */}
      <span className="text-gray-500 dark:text-gray-400 text-xs min-w-[60px]">
        [{formatTimestamp(message.timestamp)}]
      </span>

      {/* Folder and prompt for commands */}
      {shouldShowFolder && (
        <span className="text-blue-600 dark:text-blue-400 min-w-0">
          {message.current_folder}/
        </span>
      )}

      {shouldShowPrompt && (
        <span className="text-gray-600 dark:text-gray-300">
          {getSourceIcon(message.source)}
        </span>
      )}

      {/* Message content */}
      <div
        className={`flex-1 min-w-0 ${getMessageTypeStyle(message.message_type)}`}
      >
        <pre className="whitespace-pre-wrap break-words font-mono">
          {message.content}
        </pre>
      </div>
    </div>
  );
}
