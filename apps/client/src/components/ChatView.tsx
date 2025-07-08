import { invoke } from "@tauri-apps/api/core";
import { ArrowLeftIcon, SendIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Card, CardDescription, CardHeader, CardTitle } from "./ui/card";
import { ScrollArea } from "./ui/scroll-area";

interface Chat {
  id: string;
  peer_node_id: string;
  peer_alias?: string;
  last_activity: string;
  created_at: string;
  status: string;
}

interface ChatMessage {
  id: string;
  pod_id: string;
  message_text?: string;
  timestamp: string;
  direction: "sent" | "received";
  created_at: string;
}

export function ChatView() {
  const [chats, setChats] = useState<Chat[]>([]);
  const [selectedChat, setSelectedChat] = useState<Chat | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [loading, setLoading] = useState(true);
  const [messagesLoading, setMessagesLoading] = useState(false);

  const loadChats = async () => {
    try {
      setLoading(true);
      const chatList = await invoke<Chat[]>("get_chats");
      setChats(chatList);
    } catch (error) {
      console.error("Failed to load chats:", error);
    } finally {
      setLoading(false);
    }
  };

  const loadChatMessages = async (chatId: string) => {
    try {
      setMessagesLoading(true);
      const messageList = await invoke<ChatMessage[]>("get_chat_messages", {
        chatId
      });
      setMessages(messageList);
    } catch (error) {
      console.error("Failed to load chat messages:", error);
    } finally {
      setMessagesLoading(false);
    }
  };

  const handleSelectChat = (chat: Chat) => {
    setSelectedChat(chat);
    loadChatMessages(chat.id);
  };

  const formatTimestamp = (timestamp: string) => {
    return new Date(timestamp).toLocaleString();
  };

  const formatLastActivity = (timestamp: string) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

    if (diffDays === 0) {
      return date.toLocaleTimeString([], {
        hour: "2-digit",
        minute: "2-digit"
      });
    } else if (diffDays === 1) {
      return "Yesterday";
    } else if (diffDays < 7) {
      return `${diffDays} days ago`;
    } else {
      return date.toLocaleDateString();
    }
  };

  useEffect(() => {
    loadChats();
  }, []);

  if (loading) {
    return (
      <div className="p-6">
        <div className="text-center">Loading chats...</div>
      </div>
    );
  }

  return (
    <div className="flex h-full">
      {/* Chat List */}
      <div className="w-1/3 border-r border-border">
        <div className="p-4 border-b border-border">
          <div className="flex items-center justify-between">
            <h2 className="text-xl font-semibold">Chats</h2>
            <Button variant="outline" size="sm" onClick={loadChats}>
              Refresh
            </Button>
          </div>
        </div>

        <ScrollArea className="h-[calc(100vh-200px)]">
          {chats.length === 0 ? (
            <div className="p-4 text-center text-muted-foreground">
              No active chats yet.
              <br />
              Accept a message from your inbox to start a chat.
            </div>
          ) : (
            <div className="p-2">
              {chats.map((chat) => (
                <Card
                  key={chat.id}
                  className={`py-2 mb-2 cursor-pointer transition-colors hover:bg-accent ${
                    selectedChat?.id === chat.id ? "bg-accent" : ""
                  }`}
                  onClick={() => handleSelectChat(chat)}
                >
                  <CardHeader>
                    <div className="flex items-center justify-between">
                      <CardTitle className="text-sm">
                        {chat.peer_alias ||
                          `User ${chat.peer_node_id.slice(0, 8)}`}
                      </CardTitle>
                      <div className="text-xs text-muted-foreground">
                        {formatLastActivity(chat.last_activity)}
                      </div>
                    </div>
                    <CardDescription className="text-xs">
                      {chat.peer_node_id.slice(0, 16)}...
                    </CardDescription>
                  </CardHeader>
                </Card>
              ))}
            </div>
          )}
        </ScrollArea>
      </div>

      {/* Chat Messages */}
      <div className="flex-1 flex flex-col">
        {selectedChat ? (
          <>
            {/* Chat Header */}
            <div className="p-4 border-b border-border">
              <div className="flex items-center gap-3">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setSelectedChat(null)}
                  className="md:hidden"
                >
                  <ArrowLeftIcon className="h-4 w-4" />
                </Button>
                <div className="flex-1">
                  <h3 className="font-semibold">
                    {selectedChat.peer_alias ||
                      `User ${selectedChat.peer_node_id.slice(0, 8)}`}
                  </h3>
                  <p className="text-sm text-muted-foreground">
                    {selectedChat.peer_node_id.slice(0, 16)}...
                  </p>
                </div>
                <Badge variant="secondary">{selectedChat.status}</Badge>
              </div>
            </div>

            {/* Messages */}
            <ScrollArea className="flex-1 p-4">
              {messagesLoading ? (
                <div className="text-center text-muted-foreground">
                  Loading messages...
                </div>
              ) : messages.length === 0 ? (
                <div className="text-center text-muted-foreground">
                  No messages yet. Send a POD to start the conversation!
                </div>
              ) : (
                <div className="space-y-4">
                  {messages.map((message) => (
                    <div
                      key={message.id}
                      className={`flex ${message.direction === "sent" ? "justify-end" : "justify-start"}`}
                    >
                      <div
                        className={`max-w-[70%] rounded-lg px-3 py-2 ${
                          message.direction === "sent"
                            ? "bg-primary text-primary-foreground"
                            : "bg-muted"
                        }`}
                      >
                        <div className="space-y-1">
                          <div className="text-xs opacity-70">
                            POD: {message.pod_id.slice(0, 8)}...
                          </div>
                          {message.message_text && (
                            <div className="text-sm">
                              {message.message_text}
                            </div>
                          )}
                          <div className="text-xs opacity-70">
                            {formatTimestamp(message.timestamp)}
                          </div>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </ScrollArea>

            {/* Message Input Area */}
            <div className="p-4 border-t border-border">
              <div className="flex gap-2">
                <div className="flex-1 text-sm text-muted-foreground">
                  Use the debug menu to send PODs to this peer
                </div>
                <Button disabled size="sm">
                  <SendIcon className="h-4 w-4" />
                  Send
                </Button>
              </div>
            </div>
          </>
        ) : (
          <div className="flex-1 flex items-center justify-center text-muted-foreground">
            <div className="text-center">
              <h3 className="text-lg font-medium mb-2">Select a chat</h3>
              <p className="text-sm">
                Choose a chat from the list to view your conversation history.
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
