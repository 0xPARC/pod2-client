import type { SignedPod, Value } from "@pod2/pod2js";
import { invokeCommand } from "@/lib/rpc";
import type {
  Diagnostic,
  ValidateCodeResponse,
  ExecuteCodeResponse
} from "./types";

/**
 * Private key information (without the secret key)
 */
export interface PrivateKeyInfo {
  id: string;
  public_key: string; // Base58-encoded public key
  alias: string | null;
  created_at: string;
  is_default: boolean;
}

// =============================================================================
// POD Authoring Operations
// =============================================================================

/**
 * Sign a POD with the given key-value pairs
 * @param values - The key-value pairs to include in the POD
 * @returns The signed POD
 */
export async function signPod(
  values: Record<string, Value>
): Promise<SignedPod> {
  const serializedPod = await invokeCommand<string>("sign_pod", {
    serializedPodValues: JSON.stringify(values)
  });
  return JSON.parse(serializedPod);
}

// =============================================================================
// Key Management
// =============================================================================

/**
 * Get information about the default private key
 * Note: A default key is automatically created if none exists
 * @returns Private key information (without the secret key)
 */
export async function getPrivateKeyInfo(): Promise<PrivateKeyInfo> {
  return invokeCommand<PrivateKeyInfo>("get_private_key_info");
}

// =============================================================================
// Code Editor Operations
// =============================================================================

/**
 * Validate Podlang code for syntax and semantic errors
 * @param code - The Podlang code to validate
 * @returns Validation response with diagnostics
 */
export async function validateCode(code: string): Promise<Diagnostic[]> {
  const response = await invokeCommand<ValidateCodeResponse>(
    "validate_code_command",
    {
      code
    }
  );
  return response.diagnostics;
}

/**
 * Execute Podlang code against all available PODs
 * @param code - The Podlang code to execute
 * @param mock - Whether to use mock mode for faster execution
 * @returns Execution response with results
 */
export async function executeCode(
  code: string,
  mock: boolean = false
): Promise<ExecuteCodeResponse> {
  return invokeCommand<ExecuteCodeResponse>("execute_code_command", {
    code,
    mock
  });
}
