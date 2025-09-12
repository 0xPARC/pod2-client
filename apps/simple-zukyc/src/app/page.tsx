"use client";
import { Button } from "@/components/ui/button";
import { CheckCircle, XCircle } from "lucide-react";
import { useState } from "react";

export default function Home() {
  const [requested, setRequested] = useState(false);
  const [serializedPOD, setSerializedPOD] = useState<string | undefined>(
    undefined
  );
  const [verified, setVerified] = useState<boolean | undefined>(undefined);

  const request = `
    REQUEST(
      NotContains(sanctions["sanctionList"], gov["idNumber"])
      Lt(gov["dateOfBirth"], 1169909388)
      Equal(pay["startDate"], 1706367566)
      Equal(gov["socialSecurityNumber"], pay["socialSecurityNumber"])
    )
    `;

  return (
    <main className="container mx-auto py-8">
      <div className="flex flex-col gap-2">
        <h1 className="text-2xl font-bold">Simple ZuKYC</h1>
        <p>To complete KYC, you must satisfy the following POD Request:</p>
        <pre>{request}</pre>
        <div>
          <Button
            onClick={() => {
              const loc =
                "pod2://request?request=" + encodeURIComponent(request);
              console.log(loc);
              window.location.href = loc;
              setRequested(true);
            }}
          >
            POD Request
          </Button>
        </div>
        {requested && (
          <>
            <div>
              <Button
                onClick={async () => {
                  const pod = await window.navigator.clipboard.readText();
                  setSerializedPOD(pod);
                }}
              >
                Paste POD
              </Button>
            </div>
            <pre>{serializedPOD}</pre>
            {serializedPOD && (
              <div>
                <Button
                  onClick={async () => {
                    const response = await fetch("/api/verify", {
                      method: "POST",
                      body: JSON.stringify({ pod: serializedPOD })
                    });
                    const data = await response.json();
                    setVerified(data.verified);
                  }}
                >
                  Verify
                </Button>
              </div>
            )}
            {verified !== undefined && (
              <div>
                <p>
                  Verified:{" "}
                  {verified ? (
                    <CheckCircle className="text-green-500" />
                  ) : (
                    <XCircle className="text-red-500" />
                  )}
                </p>
              </div>
            )}
          </>
        )}
      </div>
    </main>
  );
}
