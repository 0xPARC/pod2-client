import type { MainPod } from "@pod2/pod2js";
import { invokeCommand } from "@/lib/rpc";

// =============================================================================
// External Integration Operations
// =============================================================================

/**
 * Submit a POD request and get back a MainPod proof
 * @param request - The POD request string
 * @returns The resulting MainPod
 */
export async function submitPodRequest(request: string): Promise<MainPod> {
  return invokeCommand<MainPod>("submit_pod_request", { request });
}
