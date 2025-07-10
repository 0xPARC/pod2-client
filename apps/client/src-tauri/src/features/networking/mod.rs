pub mod commands;

pub use commands::*;

/// Networking feature module
///
/// This module handles P2P communication including:
/// - P2P node management
/// - Sending/receiving PODs and messages
/// - Chat and inbox functionality
/// - Peer connection management
pub struct NetworkingFeature;
