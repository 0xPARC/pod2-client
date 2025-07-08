import { Ajv2019, type ValidateFunction } from "ajv/dist/2019.js";
import type { MainPod, SignedPod } from "../generated/types/pod2.d.ts";
import schema from "./schemas.json" with { type: "json" };

export type * from "../generated/types/pod2.d.ts";

const ajv = new Ajv2019({ allErrors: true, strict: false });

type Result<T> =
  | {
      success: true;
      pod: T;
    }
  | {
      success: false;
      errors: Ajv2019["errors"];
    };

export function validateMainPod(data: any): Result<MainPod> {
  if (!mainPodValidator) {
    setupValidators();
  }
  if (mainPodValidator && mainPodValidator(data)) {
    return {
      success: true,
      pod: data
    };
  }
  return {
    success: false,
    errors: ajv.errors
  };
}

export function validateSignedPod(data: any): Result<SignedPod> {
  if (!signedPodValidator) {
    setupValidators();
  }
  if (signedPodValidator && signedPodValidator(data)) {
    return {
      success: true,
      pod: data
    };
  }
  return {
    success: false,
    errors: ajv.errors
  };
}

let mainPodValidator: ValidateFunction<MainPod> | undefined;
let signedPodValidator: ValidateFunction<SignedPod> | undefined;

function setupValidators() {
  // --- AJV Setup ---

  try {
    ajv.compile(schema);
    mainPodValidator = ajv.getSchema<MainPod>("#/definitions/MainPod");
    signedPodValidator = ajv.getSchema<SignedPod>("#/definitions/SignedPod");

    if (!mainPodValidator) {
      throw new Error("Could not get validator for MainPod");
    }
    if (!signedPodValidator) {
      throw new Error("Could not get validator for SignedPod");
    }
  } catch (e) {
    console.error("Failed to compile AJV schemas:", e);
    throw e;
  }
}
