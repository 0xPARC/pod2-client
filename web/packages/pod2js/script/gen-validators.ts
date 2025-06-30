import schemas from "../src/schemas.json" with { type: "json" };
import { Ajv2019 } from "ajv/dist/2019.js";
import standaloneCode from "ajv/dist/standalone/index.js";

function compileSchemas(schemas: any) {
  const ajv = new Ajv2019({
    schemas: [schemas],
    allErrors: true,
    strict: false,
    messages: false,
    code: {
      esm: true,
      source: true,
      optimize: true,
    },
  });
  return (standaloneCode as any)(ajv, {
    AnchoredKey: "#/definitions/AnchoredKey",
    Array: "#/definitions/Array",
    CustomPredicate: "#/definitions/CustomPredicate",
    CustomPredicateBatch: "#/definitions/CustomPredicateBatch",
    CustomPredicateRef: "#/definitions/CustomPredicateRef",
    Dictionary: "#/definitions/Dictionary",
    Hash: "#/definitions/Hash",
    Key: "#/definitions/Key",
    MainPod: "#/definitions/MainPod",
    NativePredicate: "#/definitions/NativePredicate",
    Params: "#/definitions/Params",
    PodData: "#/definitions/PodData",
    PodId: "#/definitions/PodId",
    PodInfo: "#/definitions/PodInfo",
    Predicate: "#/definitions/Predicate",
    RawValue: "#/definitions/RawValue",
    Set: "#/definitions/Set",
    SignedPod: "#/definitions/SignedPod",
    SpaceInfo: "#/definitions/SpaceInfo",
    Statement: "#/definitions/Statement",
    StatementTmpl: "#/definitions/StatementTmpl",
    StatementTmplArg: "#/definitions/StatementTmplArg",
    VDSet: "#/definitions/VDSet",
    Value: "#/definitions/Value",
    ValueRef: "#/definitions/ValueRef",
    Wildcard: "#/definitions/Wildcard",
  });
}

const validators = compileSchemas(schemas);

console.log(validators);
