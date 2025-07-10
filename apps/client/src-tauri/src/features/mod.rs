pub mod authoring;
pub mod integration;
pub mod networking;
pub mod pod_management;

/// Authoring feature module
///
/// This module handles POD creation and signing including:
/// - Creating and signing new PODs
/// - Private key management
/// - POD authoring workflows
pub use authoring::*;
/// Integration feature module
///
/// This module handles external POD Request handling and protocol integration including:
/// - Processing external POD requests
/// - Deep link and URL scheme handling
/// - Protocol interoperability
/// - External application integration
pub use integration::*;
/// Networking feature module
///
/// This module handles P2P communication and messaging including:
/// - P2P node management
/// - POD sharing and exchange
/// - Chat and messaging functionality
/// - Peer discovery and communication
pub use networking::*;
/// POD management feature module
///
/// This module handles POD collection management including:
/// - Browsing and organizing PODs
/// - POD collection state management
/// - POD pinning and organization
/// - Space/folder management
pub use pod_management::*;
