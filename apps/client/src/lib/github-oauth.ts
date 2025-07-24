import { invoke } from "@tauri-apps/api/core";

export interface GitHubOAuthConfig {
  server_url: string;
}

export interface AuthUrlRequest {
  public_key: any;
  username: string;
}

export interface AuthUrlResponse {
  auth_url: string;
  state: string;
}

export interface GitHubIdentityRequest {
  code: string;
  state: string;
  username: string;
  challenge_signature: string;
}

export interface GitHubIdentityResponse {
  identity_pod: any;
}

export interface GitHubServerInfo {
  server_id: string;
  public_key: any;
}

/**
 * GitHub OAuth API client for POD2 identity server integration
 */
export class GitHubOAuthClient {
  private serverUrl: string;

  constructor(config: GitHubOAuthConfig) {
    this.serverUrl = config.server_url;
  }

  /**
   * Get server information from GitHub OAuth identity server
   */
  async getServerInfo(): Promise<GitHubServerInfo> {
    const response = await fetch(this.serverUrl);
    if (!response.ok) {
      throw new Error(`Failed to get server info: ${response.status}`);
    }
    return response.json();
  }

  /**
   * Get GitHub OAuth authorization URL
   */
  async getAuthUrl(publicKey: any, username: string): Promise<AuthUrlResponse> {
    const response = await fetch(`${this.serverUrl}/auth/github`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        public_key: publicKey,
        username: username,
      }),
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`Failed to get auth URL: ${response.status} - ${errorText}`);
    }

    return response.json();
  }

  /**
   * Complete GitHub OAuth identity verification
   */
  async completeIdentityVerification(request: GitHubIdentityRequest): Promise<GitHubIdentityResponse> {
    const response = await fetch(`${this.serverUrl}/identity`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`Failed to complete identity verification: ${response.status} - ${errorText}`);
    }

    return response.json();
  }

  /**
   * Open GitHub OAuth URL in external browser
   */
  async openAuthUrl(authUrl: string): Promise<void> {
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    await openUrl(authUrl);
  }
}

/**
 * Enhanced identity server setup that detects GitHub OAuth servers
 */
export async function setupGitHubIdentityServer(serverUrl: string): Promise<{
  server_info: GitHubServerInfo;
  is_github_server: boolean;
}> {
  // Use Tauri backend to detect GitHub OAuth server
  const is_github_server = await invoke('detect_github_oauth_server', { serverUrl }) as boolean;
  
  // Get server info
  const client = new GitHubOAuthClient({ server_url: serverUrl });
  const server_info = await client.getServerInfo();
  
  return { server_info, is_github_server };
}

/**
 * Register username with GitHub OAuth flow
 */
export async function registerWithGitHubOAuth(
  serverUrl: string,
  username: string
): Promise<{
  identity_pod: any;
  username: string;
  server_id: string;
}> {
  // Get user's public key from Tauri backend
  const privateKeyInfo = await invoke('get_private_key_info') as { public_key: any };
  const publicKey = privateKeyInfo.public_key;

  const client = new GitHubOAuthClient({ server_url: serverUrl });

  // Step 1: Get GitHub OAuth authorization URL
  const authResponse = await client.getAuthUrl(publicKey, username);

  // Step 2: Open GitHub OAuth URL in browser
  await client.openAuthUrl(authResponse.auth_url);

  // Step 3: Wait for user to complete OAuth and return authorization code
  // This would typically be handled by a callback or user input
  // For now, we'll return a promise that resolves when the user provides the code
  return new Promise((_resolve, reject) => {
    // This would be implemented with a proper OAuth callback handler
    // For now, we'll just throw an error indicating this needs to be implemented
    reject(new Error('OAuth callback handling not yet implemented. User needs to provide authorization code manually.'));
  });
}

/**
 * Extract GitHub-specific information from identity POD
 */
export function extractGitHubInfoFromIdentityPod(identityPod: any): {
  username: string;
  github_username?: string;
  github_user_id?: number;
  github_public_keys?: string[];
  github_email?: string;
  oauth_verified_at?: string;
} {
  const username = identityPod.username || 'Unknown';
  
  // Parse GitHub data dictionary
  let githubData: any = {};
  if (identityPod.github_data) {
    try {
      githubData = typeof identityPod.github_data === 'string' 
        ? JSON.parse(identityPod.github_data) 
        : identityPod.github_data;
    } catch (error) {
      console.error('Failed to parse GitHub data from identity POD:', error);
    }
  }

  return {
    username: username,
    github_username: githubData.github_username,
    github_user_id: githubData.github_user_id,
    github_public_keys: githubData.github_public_keys,
    github_email: githubData.github_email,
    oauth_verified_at: githubData.oauth_verified_at,
  };
}