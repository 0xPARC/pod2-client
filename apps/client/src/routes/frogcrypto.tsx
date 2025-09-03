import { createFileRoute } from "@tanstack/react-router";
import { FrogCrypto } from "@/components/frogcrypto/FrogCrypto";

export const Route = createFileRoute("/frogcrypto")({
  staticData: { breadcrumb: () => "FrogCrypto" },
  component: FrogCrypto
});
