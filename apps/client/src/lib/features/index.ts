// Feature-based RPC organization
//
// This module provides a clean separation of concerns by organizing
// RPC functions into feature verticals:

// Pod Management - Browsing and organizing PODs
export * as podManagement from "./pod-management";

// Networking - P2P communication and messaging
export * as networking from "./networking";

// Authoring - POD creation and signing
export * as authoring from "./authoring";

// Integration - External POD requests and protocols
export * as integration from "./integration";

// Re-export individual features for direct import
export * from "./pod-management";
export * from "./networking";
export * from "./authoring";
export * from "./integration";
