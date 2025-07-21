// Auto-generated TypeScript interfaces for Rust configuration structs

export interface FeatureConfig {
  pod_management: boolean;
  networking: boolean;
  authoring: boolean;
  integration: boolean;
  frogcrypto: boolean;
}

export interface DatabaseConfig {
  path: string;
}

export interface NetworkConfig {
  server: string;
  timeout_seconds: number;
}

export interface UiConfig {
  default_theme: string;
  default_window_width: number;
  default_window_height: number;
}

export interface LoggingConfig {
  level: string;
  console_output: boolean;
}

export interface AppConfig {
  features: FeatureConfig;
  database: DatabaseConfig;
  network: NetworkConfig;
  ui: UiConfig;
  logging: LoggingConfig;
}
