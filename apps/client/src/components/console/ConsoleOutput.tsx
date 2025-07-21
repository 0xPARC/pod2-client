import { useEffect, useRef } from "react";
import { useConsoleStore } from "../../lib/console/store";
import { ConsoleMessage } from "./ConsoleMessage";

export function ConsoleOutput() {
  const { messages, isLoading } = useConsoleStore();
  const outputRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when new messages arrive
  useEffect(() => {
    if (outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [messages]);

  return (
    <div
      ref={outputRef}
      className="flex-1 overflow-y-auto bg-gray-50 dark:bg-gray-900"
    >
      {isLoading && messages.length === 0 ? (
        <div className="flex items-center justify-center h-32">
          <div className="text-gray-500 dark:text-gray-400">
            Loading console...
          </div>
        </div>
      ) : messages.length === 0 ? (
        <div className="flex items-center justify-center h-32">
          <div className="text-gray-500 dark:text-gray-400 font-mono text-sm">
            Welcome to POD2 Console. Type 'help' to get started.
          </div>
        </div>
      ) : (
        <div className="py-2">
          {messages.map((message) => (
            <ConsoleMessage key={message.id} message={message} />
          ))}
        </div>
      )}
    </div>
  );
}
