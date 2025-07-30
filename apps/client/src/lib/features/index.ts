// Feature-based RPC organization
//
// This module provides a clean separation of concerns by organizing
// RPC functions into feature verticals:

// Pod Management - Browsing and organizing PODs
export * as podManagement from "./pod-management";

// Authoring - POD creation and signing
export * as authoring from "./authoring";

// Re-export individual features for direct import
export * from "./authoring";
export * from "./pod-management";
