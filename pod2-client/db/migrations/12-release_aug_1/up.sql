-- Clear all data from all tables due to serialization format changes
-- This migration deletes all content but preserves table structure

-- Delete from tables with foreign key dependencies first
DELETE FROM chat_messages;
DELETE FROM inbox_messages;
DELETE FROM pods;

-- Delete from remaining tables
DELETE FROM chats;
DELETE FROM private_keys;
DELETE FROM drafts;
DELETE FROM spaces;

-- Reset app setup state
UPDATE app_setup_state SET 
    setup_completed = FALSE,
    identity_server_url = NULL,
    identity_server_id = NULL,
    identity_server_public_key = NULL,
    username = NULL,
    identity_pod_id = NULL,
    completed_at = NULL
WHERE id = 1;