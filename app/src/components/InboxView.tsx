import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button } from "./ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";
import { Badge } from "./ui/badge";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { 
  Dialog, 
  DialogContent, 
  DialogDescription, 
  DialogFooter, 
  DialogHeader, 
  DialogTitle 
} from "./ui/dialog";

interface InboxMessage {
  id: string;
  from_node_id: string;
  from_alias?: string;
  pod_id: string;
  message_text?: string;
  received_at: string;
  status: 'pending' | 'accepted' | 'declined';
}

export function InboxView() {
  const [messages, setMessages] = useState<InboxMessage[]>([]);
  const [loading, setLoading] = useState(true);
  const [acceptDialog, setAcceptDialog] = useState<{
    open: boolean;
    message?: InboxMessage;
    alias: string;
  }>({ open: false, alias: '' });

  const loadInboxMessages = async () => {
    try {
      setLoading(true);
      const inboxMessages = await invoke<InboxMessage[]>('get_inbox_messages');
      setMessages(inboxMessages);
    } catch (error) {
      console.error('Failed to load inbox messages:', error);
    } finally {
      setLoading(false);
    }
  };

  const handleAcceptMessage = async (message: InboxMessage) => {
    setAcceptDialog({
      open: true,
      message,
      alias: message.from_alias || `User ${message.from_node_id.slice(0, 8)}`
    });
  };

  const confirmAcceptMessage = async () => {
    if (!acceptDialog.message) return;

    try {
      await invoke('accept_inbox_message', {
        messageId: acceptDialog.message.id,
        chatAlias: acceptDialog.alias || null
      });
      
      console.log('Message accepted successfully');
      
      // Remove the accepted message from the inbox
      setMessages(prev => prev.filter(m => m.id !== acceptDialog.message!.id));
      
      // Close dialog
      setAcceptDialog({ open: false, alias: '' });
    } catch (error) {
      console.error('Failed to accept message:', error);
    }
  };

  const formatTimestamp = (timestamp: string) => {
    return new Date(timestamp).toLocaleString();
  };

  useEffect(() => {
    loadInboxMessages();
    
    // Listen for incoming POD events
    const unlisten = listen('p2p-pod-received', () => {
      console.log('New POD received, refreshing inbox...');
      loadInboxMessages();
    });
    
    return () => {
      unlisten.then(f => f());
    };
  }, []);

  if (loading) {
    return (
      <div className="p-6">
        <div className="text-center">Loading inbox...</div>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold">Inbox</h2>
        <Button variant="outline" onClick={loadInboxMessages}>
          Refresh
        </Button>
      </div>

      {messages.length === 0 ? (
        <Card>
          <CardContent className="p-6 text-center text-muted-foreground">
            No pending messages in your inbox.
          </CardContent>
        </Card>
      ) : (
        <div className="space-y-4">
          {messages.map((message) => (
            <Card key={message.id} className="relative">
              <CardHeader>
                <div className="flex items-center justify-between">
                  <div>
                    <CardTitle className="text-lg">
                      POD from {message.from_alias || 'Unknown Sender'}
                    </CardTitle>
                    <CardDescription>
                      Node ID: {message.from_node_id.slice(0, 16)}...
                    </CardDescription>
                  </div>
                  <Badge variant="secondary">
                    {message.status}
                  </Badge>
                </div>
              </CardHeader>
              
              <CardContent className="space-y-4">
                <div className="grid grid-cols-1 gap-2 text-sm">
                  <div>
                    <span className="font-medium">POD ID:</span>{' '}
                    <span className="font-mono text-xs">{message.pod_id}</span>
                  </div>
                  <div>
                    <span className="font-medium">Received:</span>{' '}
                    {formatTimestamp(message.received_at)}
                  </div>
                  {message.message_text && (
                    <div>
                      <span className="font-medium">Message:</span>{' '}
                      <span className="italic">"{message.message_text}"</span>
                    </div>
                  )}
                </div>
                
                <div className="flex gap-2">
                  <Button 
                    onClick={() => handleAcceptMessage(message)}
                    className="flex-1"
                  >
                    Accept & Start Chat
                  </Button>
                  <Button 
                    variant="outline" 
                    className="flex-1"
                    disabled
                  >
                    Decline
                  </Button>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}

      <Dialog open={acceptDialog.open} onOpenChange={(open) => !open && setAcceptDialog({ open: false, alias: '' })}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Accept Message</DialogTitle>
            <DialogDescription>
              Accept this POD message and start a chat with the sender.
            </DialogDescription>
          </DialogHeader>
          
          <div className="space-y-4">
            <div>
              <Label htmlFor="chat-alias">Chat Name</Label>
              <Input
                id="chat-alias"
                value={acceptDialog.alias}
                onChange={(e) => setAcceptDialog(prev => ({ ...prev, alias: e.target.value }))}
                placeholder="Enter a name for this chat"
              />
            </div>
          </div>
          
          <DialogFooter>
            <Button variant="outline" onClick={() => setAcceptDialog({ open: false, alias: '' })}>
              Cancel
            </Button>
            <Button onClick={confirmAcceptMessage}>
              Accept & Start Chat
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}