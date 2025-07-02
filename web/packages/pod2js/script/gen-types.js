#!/usr/bin/env node

import { execSync } from "child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  writeFileSync
} from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const packageRoot = join(__dirname, "..");
const typesDir = join(packageRoot, "generated", "types");
const schemaFile = join(packageRoot, "src", "schemas.json");
const outputFile = join(typesDir, "pod2.d.ts");
const tempFile = join(typesDir, "pod2.d.ts.tmp");

try {
  console.log("Generating TypeScript types...");

  // Ensure directory exists
  mkdirSync(typesDir, { recursive: true });

  // Check if schema file exists
  if (!existsSync(schemaFile)) {
    console.error("Schema file not found:", schemaFile);
    process.exit(1);
  }

  // Generate types to temporary file
  const command = `json2ts --no-additionalProperties "${schemaFile}"`;
  const output = execSync(command, { encoding: "utf8" });

  // Check if content has changed (only if output file exists)
  let contentChanged = true;
  if (existsSync(outputFile)) {
    const existingContent = readFileSync(outputFile, "utf8");
    contentChanged = existingContent !== output;
  }

  if (contentChanged) {
    // Write to temporary file first
    writeFileSync(tempFile, output, "utf8");

    // Atomic move to final location
    renameSync(tempFile, outputFile);
    console.log("Types generated successfully");
  } else {
    console.log("Types are up to date, no changes needed");
  }
} catch (error) {
  console.error("Error generating types:", error.message);
  console.error("This is likely due to:");
  console.error("- Invalid JSON schema in", schemaFile);
  console.error("- Missing json-schema-to-typescript dependency");
  process.exit(1);
}
