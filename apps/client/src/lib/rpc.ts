import { MainPod, SignedPod, Value } from "@pod2/pod2js";
import { invoke } from "@tauri-apps/api/core";

export async function signPod(
  values: Record<string, Value>
): Promise<SignedPod> {
  const serializedPod = (await invoke("sign_pod", {
    serializedPodValues: JSON.stringify(values)
  })) as string;
  return JSON.parse(serializedPod);
}

export async function importPod(
  pod: SignedPod | MainPod,
  label?: string
): Promise<void> {
  const type = pod.podType;
  console.log(type);
  return invoke("import_pod", {
    serializedPod: JSON.stringify(pod),
    podType: type[1],
    label: label
  });
}
