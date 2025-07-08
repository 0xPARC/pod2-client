import { MainPod } from "@pod2/pod2-node";

export async function POST(request: Request): Promise<Response> {
  const { pod } = await request.json();
  const mainPod = MainPod.deserialize(pod);
  const verified = mainPod.verify();
  return Response.json({ verified });
}
