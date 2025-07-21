use std::collections::{HashMap, VecDeque};

use chrono::Utc;
use pod2_db::store;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{Mutex, RwLock};

use super::{
    aliases::AliasRegistry,
    binding::ParameterBinder,
    parser::parse_console_command,
    types::{ConsoleEvent, ConsoleMessage, ConsoleState, MessageSource, MessageType},
};
use crate::{features::authoring::commands::execute_code_command, AppState};

/// Format size in bytes to human-readable format
fn format_size_bytes(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    const THRESHOLD: f64 = 1024.0;

    if bytes == 0 {
        return "0B".to_string();
    }

    let bytes_f = bytes as f64;
    let mut size = bytes_f;
    let mut unit_index = 0;

    while size >= THRESHOLD && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{}{}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1}{}", size, UNITS[unit_index])
    }
}

/// Main console service managing state and operations
pub struct ConsoleService {
    /// Ring buffer for recent messages (fast access)
    message_buffer: RwLock<VecDeque<ConsoleMessage>>,
    /// Current working folder
    current_folder: RwLock<String>,
    /// Command history
    command_history: RwLock<VecDeque<String>>,
    /// Message counter for unique IDs
    message_counter: RwLock<u64>,
    /// App handle for events
    app_handle: AppHandle,
    /// Alias registry for TOML-defined commands
    alias_registry: RwLock<AliasRegistry>,
    /// Parameter binder for wildcard substitution
    parameter_binder: ParameterBinder,
}

impl ConsoleService {
    /// Create new console service
    pub fn new(app_handle: AppHandle) -> Self {
        // Load aliases from default locations
        let alias_registry = AliasRegistry::load_default(&app_handle).unwrap_or_else(|e| {
            log::warn!("Failed to load aliases: {}", e);
            AliasRegistry::new()
        });

        Self {
            message_buffer: RwLock::new(VecDeque::with_capacity(1000)),
            current_folder: RwLock::new("default".to_string()),
            command_history: RwLock::new(VecDeque::with_capacity(100)),
            message_counter: RwLock::new(0),
            app_handle,
            alias_registry: RwLock::new(alias_registry),
            parameter_binder: ParameterBinder::new(),
        }
    }

    /// Show welcome message with ASCII art
    pub async fn show_welcome(&self) {
        let ascii_art = r#" _       __     __                             __               
| |     / /__  / /________  ____ ___  ___     / /_____          
| | /| / / _ \/ / ___/ __ \/ __ `__ \/ _ \   / __/ __ \         
| |/ |/ /  __/ / /__/ /_/ / / / / / /  __/  / /_/ /_/ /         
|__/|__/\___/_/\___/\____/_/ /_/ /_/\___/   \__/\____/          
  / /_/ /_  ___     / __ \____  ____/ /___  ___  / /_           
 / __/ __ \/ _ \   / /_/ / __ \/ __  / __ \/ _ \/ __/           
/ /_/ / / /  __/  / ____/ /_/ / /_/ / / / /  __/ /_             
\__/_/ /_/\___/  /_/    \____/\__,_/_/ /_/\___/\__/             
                                                                "#;

        self.add_message(
            MessageType::SystemEvent,
            ascii_art.to_string(),
            MessageSource::System,
        )
        .await;

        self.add_message(
            MessageType::SystemEvent,
            "🚀 POD2 Console initialized - Type 'help' for available commands".to_string(),
            MessageSource::System,
        )
        .await;
    }

    /// Add a message to the console
    pub async fn add_message(
        &self,
        message_type: MessageType,
        content: String,
        source: MessageSource,
    ) -> u64 {
        let mut counter = self.message_counter.write().await;
        let mut buffer = self.message_buffer.write().await;
        let current_folder = self.current_folder.read().await.clone();

        let id = *counter;
        *counter += 1;

        let message = ConsoleMessage {
            id,
            timestamp: Utc::now(),
            message_type,
            content,
            source,
            current_folder,
        };

        // Add to ring buffer
        if buffer.len() >= 1000 {
            buffer.pop_front();
        }
        buffer.push_back(message.clone());

        // TODO: Also store in database for persistence

        // Emit update event
        // log::debug!("Backend: Emitting console-updated event for message: {:?}", message); // Debug: can re-enable if needed
        let _ = self.app_handle.emit("console-updated", &message);

        id
    }

    /// Execute a console command
    pub async fn execute_command(&self, input: String) -> Result<String, String> {
        // log::debug!("Console execute_command called with input: '{}'", input); // Debug: can re-enable if needed
        // Add command to history
        let mut history = self.command_history.write().await;
        if history.len() >= 100 {
            history.pop_front();
        }
        history.push_back(input.clone());
        drop(history);

        // Log the command
        self.add_message(MessageType::Command, input.clone(), MessageSource::Console)
            .await;

        // Parse the command
        let command = {
            let alias_registry = self.alias_registry.read().await;
            parse_console_command(&input, Some(&*alias_registry))
        };

        match command {
            Ok(command) => match command {
                super::types::ConsoleCommand::BuiltIn { name, args } => {
                    self.execute_builtin_command(name, args).await
                }
                super::types::ConsoleCommand::Alias { name, params } => {
                    self.execute_alias_command(name, params).await
                }
                super::types::ConsoleCommand::Exec { code } => self.execute_raw_podlang(code).await,
            },
            Err(e) => {
                self.add_message(
                    MessageType::Error,
                    format!("Parse error: {}", e),
                    MessageSource::Console,
                )
                .await;
                Err(e)
            }
        }
    }

    /// Execute built-in command
    async fn execute_builtin_command(
        &self,
        name: String,
        args: Vec<String>,
    ) -> Result<String, String> {
        match name.as_str() {
            "pwd" => {
                let folder = self.current_folder.read().await.clone();
                self.add_message(
                    MessageType::CommandResult,
                    format!("/{}", folder),
                    MessageSource::Console,
                )
                .await;
                Ok(folder)
            }
            "cd" => {
                if args.is_empty() {
                    return Err("cd requires a folder name".to_string());
                }
                let new_folder = args[0].clone();
                *self.current_folder.write().await = new_folder.clone();
                self.add_message(
                    MessageType::CommandResult,
                    format!("Changed to folder: {}/", new_folder),
                    MessageSource::Console,
                )
                .await;
                Ok(new_folder)
            }
            "ls" => {
                let folder_filter = if args.is_empty() {
                    None
                } else {
                    Some(args[0].clone())
                };

                // Access the database through AppHandle
                let app_state_mutex: tauri::State<Mutex<AppState>> = self.app_handle.state();
                let app_state = app_state_mutex.lock().await;

                match store::list_all_pods(&app_state.db).await {
                    Ok(pod_infos) => {
                        let current_folder = self.current_folder.read().await.clone();

                        // Filter PODs by current folder or specified folder
                        let target_folder = folder_filter.unwrap_or(current_folder);
                        let filtered_pods: Vec<_> = pod_infos
                            .into_iter()
                            .filter(|pod| pod.space == target_folder)
                            .collect();

                        if filtered_pods.is_empty() {
                            self.add_message(
                                MessageType::CommandResult,
                                format!("No PODs found in folder '{}'", target_folder),
                                MessageSource::Console,
                            )
                            .await;
                        } else {
                            let mut output = format!("PODs in folder '{}':\n", target_folder);
                            for pod in &filtered_pods {
                                // Truncate POD ID to 8 chars
                                let short_id = &pod.id[..8.min(pod.id.len())];

                                // Determine POD type and pad to 6 chars
                                let pod_type = match pod.data {
                                    store::PodData::Signed(_) => "Signed",
                                    store::PodData::Main(_) => "Main",
                                };
                                let padded_type = format!("{:<6}", pod_type);

                                // Format size and pad to 8 chars for alignment
                                let size_str = format_size_bytes(pod.size_bytes);
                                let padded_size = format!("{:>8}", size_str);

                                // Use label as-is, or leave blank if none
                                let label = pod.label.as_ref().map(|l| l.as_str()).unwrap_or("");

                                output.push_str(&format!(
                                    "  {} {} {} {}\n",
                                    short_id, padded_type, padded_size, label
                                ));
                            }
                            output.push_str(&format!("\nTotal: {} PODs", filtered_pods.len()));

                            self.add_message(
                                MessageType::CommandResult,
                                output,
                                MessageSource::Console,
                            )
                            .await;
                        }
                        Ok(format!("Listed {} PODs", filtered_pods.len()))
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to list PODs: {}", e);
                        self.add_message(
                            MessageType::Error,
                            error_msg.clone(),
                            MessageSource::Console,
                        )
                        .await;
                        Err(error_msg)
                    }
                }
            }
            "mv" => {
                if args.len() != 2 {
                    let error_msg = "mv requires exactly 2 arguments: <pod_id> <target_folder>";
                    self.add_message(
                        MessageType::Error,
                        error_msg.to_string(),
                        MessageSource::Console,
                    )
                    .await;
                    return Err(error_msg.to_string());
                }

                let (pod_id, from_space, to_space) =
                    self.parse_pod_space_args(&args[0], &args[1]).await;

                // Access the database through AppHandle
                let app_state_mutex: tauri::State<Mutex<AppState>> = self.app_handle.state();
                let mut app_state = app_state_mutex.lock().await;

                match store::move_pod(&app_state.db, &pod_id, &from_space, &to_space).await {
                    Ok(()) => {
                        let success_msg = format!(
                            "Moved POD {} from folder '{}' to '{}'",
                            pod_id, from_space, to_space
                        );
                        self.add_message(
                            MessageType::CommandResult,
                            success_msg.clone(),
                            MessageSource::Console,
                        )
                        .await;

                        // Trigger UI state sync to update POD lists
                        if let Err(sync_error) = app_state.trigger_state_sync().await {
                            log::warn!("Failed to sync state after moving POD: {}", sync_error);
                        }

                        Ok(success_msg)
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to move POD: {}", e);
                        self.add_message(
                            MessageType::Error,
                            error_msg.clone(),
                            MessageSource::Console,
                        )
                        .await;
                        Err(error_msg)
                    }
                }
            }
            "rm" => {
                if args.len() != 1 {
                    let error_msg = "rm requires exactly 1 argument: <pod_id> or <folder>/<pod_id>";
                    self.add_message(
                        MessageType::Error,
                        error_msg.to_string(),
                        MessageSource::Console,
                    )
                    .await;
                    return Err(error_msg.to_string());
                }

                let (pod_id, space) = self.parse_pod_arg(&args[0]).await;

                // Access the database through AppHandle
                let app_state_mutex: tauri::State<Mutex<AppState>> = self.app_handle.state();
                let mut app_state = app_state_mutex.lock().await;

                match store::delete_pod(&app_state.db, &space, &pod_id).await {
                    Ok(deleted_count) => {
                        let success_msg = if deleted_count > 0 {
                            format!("Deleted POD {} from folder '{}'", pod_id, space)
                        } else {
                            format!("POD {} not found in folder '{}'", pod_id, space)
                        };
                        self.add_message(
                            MessageType::CommandResult,
                            success_msg.clone(),
                            MessageSource::Console,
                        )
                        .await;

                        // Trigger UI state sync to update POD lists (only if POD was actually deleted)
                        if deleted_count > 0 {
                            if let Err(sync_error) = app_state.trigger_state_sync().await {
                                log::warn!(
                                    "Failed to sync state after deleting POD: {}",
                                    sync_error
                                );
                            }
                        }

                        Ok(success_msg)
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to delete POD: {}", e);
                        self.add_message(
                            MessageType::Error,
                            error_msg.clone(),
                            MessageSource::Console,
                        )
                        .await;
                        Err(error_msg)
                    }
                }
            }
            "cp" => {
                if args.len() != 2 {
                    let error_msg = "cp requires exactly 2 arguments: <pod_id> <target_folder>";
                    self.add_message(
                        MessageType::Error,
                        error_msg.to_string(),
                        MessageSource::Console,
                    )
                    .await;
                    return Err(error_msg.to_string());
                }

                let (pod_id, from_space, to_space) =
                    self.parse_pod_space_args(&args[0], &args[1]).await;

                // Access the database through AppHandle
                let app_state_mutex: tauri::State<Mutex<AppState>> = self.app_handle.state();
                let mut app_state = app_state_mutex.lock().await;

                match store::copy_pod(&app_state.db, &pod_id, &from_space, &to_space).await {
                    Ok(()) => {
                        let success_msg = format!(
                            "Copied POD {} from folder '{}' to '{}'",
                            pod_id, from_space, to_space
                        );
                        self.add_message(
                            MessageType::CommandResult,
                            success_msg.clone(),
                            MessageSource::Console,
                        )
                        .await;

                        // Trigger UI state sync to update POD lists
                        if let Err(sync_error) = app_state.trigger_state_sync().await {
                            log::warn!("Failed to sync state after copying POD: {}", sync_error);
                        }

                        Ok(success_msg)
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to copy POD: {}", e);
                        self.add_message(
                            MessageType::Error,
                            error_msg.clone(),
                            MessageSource::Console,
                        )
                        .await;
                        Err(error_msg)
                    }
                }
            }
            "clear" => {
                let mut buffer = self.message_buffer.write().await;
                buffer.clear();
                let _ = self.app_handle.emit("console-cleared", ());
                Ok("Console cleared".to_string())
            }
            "help" => {
                let help_text = r#"Built-in commands:
  pwd                    - Show current folder
  cd <folder>           - Change current folder  
  ls [folder]           - List PODs
  mv <pod_id> <folder>  - Move POD to different folder
  rm <pod_id>           - Delete POD from current space
  rm <folder>/<pod_id>  - Delete POD from specific folder
  cp <pod_id> <folder>  - Copy POD to different folder
  cat <pod_id>          - Display POD contents
  cat <folder>/<pod_id> - Display POD contents from specific folder
  mkdir <folder>        - Create new folder/space
  rmdir <folder>        - Delete empty folder/space
  clear                 - Clear console
  help                  - Show this help
  aliases               - List available aliases  
  reload                - Reload aliases from config file
  
Alias commands:
  <alias> param=value   - Execute alias with parameters
  
Example:
  zukyc gov=pod_123 age_threshold=946684800"#;

                self.add_message(
                    MessageType::CommandResult,
                    help_text.to_string(),
                    MessageSource::Console,
                )
                .await;
                Ok("Help displayed".to_string())
            }
            "aliases" => {
                let alias_registry = self.alias_registry.read().await;
                let aliases = alias_registry.list_aliases();

                let output = if aliases.is_empty() {
                    // Show the primary config directory path
                    let config_path = match self.app_handle.path().app_config_dir() {
                        Ok(config_dir) => config_dir.join("aliases.toml"),
                        Err(_) => std::path::PathBuf::from("aliases.toml"),
                    };
                    format!(
                        "No aliases loaded. Place an aliases.toml file at:\n  {}",
                        config_path.display()
                    )
                } else {
                    let mut output = format!("Loaded aliases ({}):\n", aliases.len());
                    for alias_name in aliases {
                        if let Some(alias) = alias_registry.get_alias(alias_name) {
                            output.push_str(&format!(
                                "  {} - parameters: {}\n",
                                alias_name,
                                if alias.parameters.is_empty() {
                                    "none".to_string()
                                } else {
                                    format!("{} (optional)", alias.parameters.join(", "))
                                }
                            ));
                        }
                    }
                    output.push_str(&format!("\nConfig: {}", alias_registry.get_config_status()));
                    output
                };

                self.add_message(MessageType::CommandResult, output, MessageSource::Console)
                    .await;
                Ok("Listed aliases".to_string())
            }
            "reload" => {
                let mut alias_registry = self.alias_registry.write().await;
                match AliasRegistry::load_default(&self.app_handle) {
                    Ok(new_registry) => {
                        *alias_registry = new_registry;
                        let alias_count = alias_registry.aliases.len();
                        self.add_message(
                            MessageType::CommandResult,
                            format!("Reloaded configuration. {} aliases loaded.", alias_count),
                            MessageSource::Console,
                        )
                        .await;
                        Ok(format!("Reloaded {} aliases", alias_count))
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to reload configuration: {}", e);
                        self.add_message(
                            MessageType::Error,
                            error_msg.clone(),
                            MessageSource::Console,
                        )
                        .await;
                        Err(error_msg)
                    }
                }
            }
            "cat" => {
                if args.len() != 1 {
                    let error_msg =
                        "cat requires exactly 1 argument: <pod_id> or <folder>/<pod_id>";
                    self.add_message(
                        MessageType::Error,
                        error_msg.to_string(),
                        MessageSource::Console,
                    )
                    .await;
                    return Err(error_msg.to_string());
                }

                let (pod_id, space) = self.parse_pod_arg(&args[0]).await;

                // Access the database through AppHandle
                let app_state_mutex: tauri::State<Mutex<AppState>> = self.app_handle.state();
                let app_state = app_state_mutex.lock().await;

                match store::get_pod(&app_state.db, &space, &pod_id).await {
                    Ok(Some(pod_info)) => {
                        // Format the POD using the Display trait
                        let pod_content = match &pod_info.data {
                            store::PodData::Signed(signed_pod_helper) => {
                                match pod2::frontend::SignedPod::try_from(signed_pod_helper.clone())
                                {
                                    Ok(signed_pod) => format!("{}", signed_pod),
                                    Err(e) => format!("Error displaying POD: {}", e),
                                }
                            }
                            store::PodData::Main(main_pod_helper) => {
                                match pod2::frontend::MainPod::try_from(main_pod_helper.clone()) {
                                    Ok(main_pod) => format!("{}", main_pod),
                                    Err(e) => format!("Error displaying POD: {}", e),
                                }
                            }
                        };

                        self.add_message(
                            MessageType::CommandResult,
                            pod_content,
                            MessageSource::Console,
                        )
                        .await;
                        Ok(format!("Displayed POD {}", pod_id))
                    }
                    Ok(None) => {
                        let error_msg = format!("POD {} not found in folder '{}'", pod_id, space);
                        self.add_message(
                            MessageType::Error,
                            error_msg.clone(),
                            MessageSource::Console,
                        )
                        .await;
                        Err(error_msg)
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to get POD: {}", e);
                        self.add_message(
                            MessageType::Error,
                            error_msg.clone(),
                            MessageSource::Console,
                        )
                        .await;
                        Err(error_msg)
                    }
                }
            }
            "mkdir" => {
                if args.len() != 1 {
                    let error_msg = "mkdir requires exactly 1 argument: <folder_name>";
                    self.add_message(
                        MessageType::Error,
                        error_msg.to_string(),
                        MessageSource::Console,
                    )
                    .await;
                    return Err(error_msg.to_string());
                }

                let folder_name = &args[0];

                // Access the database through AppHandle
                let app_state_mutex: tauri::State<Mutex<AppState>> = self.app_handle.state();
                let mut app_state = app_state_mutex.lock().await;

                match store::create_space(&app_state.db, folder_name).await {
                    Ok(()) => {
                        let success_msg = format!("Created folder: {}/", folder_name);
                        self.add_message(
                            MessageType::CommandResult,
                            success_msg.clone(),
                            MessageSource::Console,
                        )
                        .await;

                        // Trigger UI state sync to update folder list
                        if let Err(sync_error) = app_state.trigger_state_sync().await {
                            log::warn!(
                                "Failed to sync state after creating folder: {}",
                                sync_error
                            );
                        }

                        Ok(success_msg)
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to create folder '{}': {}", folder_name, e);
                        self.add_message(
                            MessageType::Error,
                            error_msg.clone(),
                            MessageSource::Console,
                        )
                        .await;
                        Err(error_msg)
                    }
                }
            }
            "rmdir" => {
                if args.len() != 1 {
                    let error_msg = "rmdir requires exactly 1 argument: <folder_name>";
                    self.add_message(
                        MessageType::Error,
                        error_msg.to_string(),
                        MessageSource::Console,
                    )
                    .await;
                    return Err(error_msg.to_string());
                }

                let folder_name = &args[0];

                // Don't allow deleting the default folder
                if folder_name == "default" {
                    let error_msg = "Cannot delete the default folder";
                    self.add_message(
                        MessageType::Error,
                        error_msg.to_string(),
                        MessageSource::Console,
                    )
                    .await;
                    return Err(error_msg.to_string());
                }

                // Access the database through AppHandle
                let app_state_mutex: tauri::State<Mutex<AppState>> = self.app_handle.state();
                let mut app_state = app_state_mutex.lock().await;

                // Check if folder has any PODs
                match store::list_pods(&app_state.db, folder_name).await {
                    Ok(pods) => {
                        if !pods.is_empty() {
                            let error_msg = format!(
                                "Cannot delete folder '{}': contains {} PODs",
                                folder_name,
                                pods.len()
                            );
                            self.add_message(
                                MessageType::Error,
                                error_msg.clone(),
                                MessageSource::Console,
                            )
                            .await;
                            return Err(error_msg);
                        }
                    }
                    Err(_) => {
                        // Folder probably doesn't exist, let delete_space handle it
                    }
                }

                match store::delete_space(&app_state.db, folder_name).await {
                    Ok(deleted_count) => {
                        if deleted_count > 0 {
                            let success_msg = format!("Deleted folder: {}/", folder_name);
                            self.add_message(
                                MessageType::CommandResult,
                                success_msg.clone(),
                                MessageSource::Console,
                            )
                            .await;

                            // Trigger UI state sync to update folder list
                            if let Err(sync_error) = app_state.trigger_state_sync().await {
                                log::warn!(
                                    "Failed to sync state after deleting folder: {}",
                                    sync_error
                                );
                            }

                            Ok(success_msg)
                        } else {
                            let error_msg = format!("Folder '{}' not found", folder_name);
                            self.add_message(
                                MessageType::Error,
                                error_msg.clone(),
                                MessageSource::Console,
                            )
                            .await;
                            Err(error_msg)
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to delete folder '{}': {}", folder_name, e);
                        self.add_message(
                            MessageType::Error,
                            error_msg.clone(),
                            MessageSource::Console,
                        )
                        .await;
                        Err(error_msg)
                    }
                }
            }
            _ => {
                let error = format!("Unknown command: {}", name);
                self.add_message(MessageType::Error, error.clone(), MessageSource::Console)
                    .await;
                Err(error)
            }
        }
    }

    /// Execute alias command
    async fn execute_alias_command(
        &self,
        name: String,
        params: HashMap<String, String>,
    ) -> Result<String, String> {
        let alias_registry = self.alias_registry.read().await;

        // Check if alias exists
        let alias = match alias_registry.get_alias(&name) {
            Some(alias) => alias,
            None => {
                let available_aliases = alias_registry.list_aliases();
                let error_msg = if available_aliases.is_empty() {
                    format!("Unknown alias '{}'. No aliases are currently loaded.", name)
                } else {
                    format!(
                        "Unknown alias '{}'. Available aliases: {}",
                        name,
                        available_aliases.join(", ")
                    )
                };
                self.add_message(
                    MessageType::Error,
                    error_msg.clone(),
                    MessageSource::Console,
                )
                .await;
                return Err(error_msg);
            }
        };

        // Bind parameters to the alias template
        match self.parameter_binder.bind_parameters(alias, &params) {
            Ok(resolved_code) => {
                self.add_message(
                    MessageType::CommandResult,
                    format!(
                        "Executing alias '{}' with resolved code:\n{}",
                        name, resolved_code
                    ),
                    MessageSource::Console,
                )
                .await;

                // Execute the resolved Podlang code
                self.execute_raw_podlang(resolved_code).await
            }
            Err(e) => {
                let error_msg = format!("Parameter binding failed for alias '{}': {}", name, e);
                self.add_message(
                    MessageType::Error,
                    error_msg.clone(),
                    MessageSource::Console,
                )
                .await;
                Err(error_msg)
            }
        }
    }

    /// Execute raw Podlang code
    async fn execute_raw_podlang(&self, code: String) -> Result<String, String> {
        // Get the app state for code execution
        let app_state_mutex: tauri::State<Mutex<AppState>> = self.app_handle.state();

        match execute_code_command(app_state_mutex, code.clone(), false).await {
            Ok(response) => {
                // Format the results for console display
                let output = format!("✓ Execution successful!\n\n{}", response.main_pod);

                self.add_message(MessageType::CommandResult, output, MessageSource::Console)
                    .await;

                Ok("Execution successful".to_string())
            }
            Err(e) => {
                let error_msg = format!("Execution failed: {}", e);
                self.add_message(
                    MessageType::Error,
                    error_msg.clone(),
                    MessageSource::Console,
                )
                .await;
                Err(error_msg)
            }
        }
    }

    /// Get recent messages from ring buffer
    pub async fn get_messages(&self, limit: Option<usize>) -> Vec<ConsoleMessage> {
        let buffer = self.message_buffer.read().await;
        let limit = limit.unwrap_or(100);

        buffer.iter().rev().take(limit).rev().cloned().collect()
    }

    /// Get current console state
    pub async fn get_state(&self) -> ConsoleState {
        let current_folder = self.current_folder.read().await.clone();
        let buffer = self.message_buffer.read().await;
        let alias_registry = self.alias_registry.read().await;

        ConsoleState {
            current_folder,
            total_message_count: buffer.len() as u64,
            aliases_loaded: !alias_registry.aliases.is_empty(),
            config_file_status: alias_registry.get_config_status(),
        }
    }

    /// Log GUI event
    pub async fn log_gui_event(&self, event: ConsoleEvent) {
        self.add_message(MessageType::GuiEvent, event.message, event.source)
            .await;
    }

    /// Helper function for logging POD operations from other modules
    pub async fn log_pod_operation(&self, message: String) {
        self.add_message(MessageType::GuiEvent, message, MessageSource::Gui)
            .await;
    }

    /// Helper function for logging system events from other modules
    pub async fn log_system_event(&self, message: String) {
        self.add_message(MessageType::SystemEvent, message, MessageSource::System)
            .await;
    }

    /// Helper function for logging errors from other modules
    pub async fn log_error_event(&self, message: String) {
        self.add_message(MessageType::Error, message, MessageSource::System)
            .await;
    }

    /// Get command history
    pub async fn get_command_history(&self) -> Vec<String> {
        let history = self.command_history.read().await;
        history.iter().cloned().collect()
    }

    /// Parse a single POD argument that may include a space prefix (e.g., "space/pod_id" or just "pod_id")
    /// Returns (pod_id, space) where space defaults to current folder if not specified
    async fn parse_pod_arg(&self, arg: &str) -> (String, String) {
        if let Some(slash_pos) = arg.find('/') {
            // Format: space/pod_id
            let space = arg[..slash_pos].to_string();
            let pod_id = arg[slash_pos + 1..].to_string();
            (pod_id, space)
        } else {
            // Format: pod_id (use current folder as space)
            let current_folder = self.current_folder.read().await.clone();
            (arg.to_string(), current_folder)
        }
    }

    /// Parse POD and space arguments for two-argument commands like mv and cp
    /// First arg can be "space/pod_id" or "pod_id", second arg is target space
    /// Returns (pod_id, from_space, to_space)
    async fn parse_pod_space_args(
        &self,
        pod_arg: &str,
        target_space: &str,
    ) -> (String, String, String) {
        let (pod_id, from_space) = self.parse_pod_arg(pod_arg).await;
        (pod_id, from_space, target_space.to_string())
    }
}
