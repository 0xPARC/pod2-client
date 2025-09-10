import { Ajv2019, type ValidateFunction } from "ajv/dist/2019.js";
import type { MainPod, SignedDict } from "../generated/types/pod2.d.ts";
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

export function validateSignedDict(data: any): Result<SignedDict> {
  if (!signedDictValidator) {
    setupValidators();
  }
  if (signedDictValidator && signedDictValidator(data)) {
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
let signedDictValidator: ValidateFunction<SignedDict> | undefined;

function setupValidators() {
  // --- AJV Setup ---

  try {
    ajv.compile(schema);
    mainPodValidator = ajv.getSchema<MainPod>("#/definitions/MainPod");
    signedDictValidator = ajv.getSchema<SignedDict>("#/definitions/SignedDict");

    if (!mainPodValidator) {
      throw new Error("Could not get validator for MainPod");
    }
    if (!signedDictValidator) {
      throw new Error("Could not get validator for SignedDict");
    }
  } catch (e) {
    console.error("Failed to compile AJV schemas:", e);
    throw e;
  }
}
