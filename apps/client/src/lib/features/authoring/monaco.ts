// Monaco Editor Setup and Language Configuration
//
// This module configures Monaco editor for POD development including:
// - Custom Podlang language syntax highlighting
// - Diagnostic marker conversion
// - Editor setup utilities

import type { languages } from "monaco-editor";
import * as monaco from "monaco-editor";
import type { Diagnostic } from "./types";

/**
 * Podlang language definition for Monaco editor syntax highlighting
 * Based on the Monarch tokenizer system
 */
export const PodlangMonarchLanguage: languages.IMonarchLanguage = {
  // Set defaultToken to invalid to see if you do not cover all cases
  defaultToken: "invalid",

  keywords: [
    "REQUEST",
    "AND",
    "OR",
    "ValueOf",
    "Equal",
    "NotEqual",
    "Gt",
    "GtEq",
    "Lt",
    "LtEq",
    "Contains",
    "NotContains",
    "SumOf",
    "ProductOf",
    "MaxOf",
    "HashOf",
    "DictContains",
    "DictNotContains",
    "ArrayContains",
    "SetContains",
    "SetNotContains"
  ],

  operators: ["="],

  symbols: /[=,:(){}\\[\\]#?]+/, // Includes '?' for variables and '#'

  escapes:
    /\\\\(?:[abfnrtv\\"']|x[0-9A-Fa-f]{1,4}|u[0-9A-Fa-f]{4}|U[0-9A-Fa-f]{8})/,

  tokenizer: {
    root: [
      // Whitespace and comments
      { include: "@whitespace" },

      // Specific keywords with symbols or specific prefixes first
      [/private:/, "keyword"],

      // --- Literals ---
      // Boolean Literals (placed before general identifiers)
      [/\\b(true|false)\\b/, "constant.language.boolean.Podlang"],

      // Number Literals
      [/0[xX][0-9a-fA-F]+/, "constant.numeric.hex.Podlang"], // Hex
      [/-?\\d+/, "constant.numeric.integer.Podlang"], // Integer

      // String Literals
      [
        /"/,
        {
          token: "string.quoted.double.Podlang",
          bracket: "@open",
          next: "@string_double"
        }
      ],
      [
        /'/,
        {
          token: "string.quoted.single.Podlang",
          bracket: "@open",
          next: "@string_single"
        }
      ],

      // Variables: start with '?', use explicit char class
      [/\\?[a-zA-Z_][a-zA-Z0-9_]*/, "variable.name.Podlang"],

      // Identifiers and keywords: general case
      [
        /[a-zA-Z_][a-zA-Z0-9_]*/,
        {
          cases: {
            "@keywords": "keyword.control.Podlang",
            "@default": "identifier.Podlang"
          }
        }
      ],

      // Delimiters and brackets
      [/[\\[\\]]/, "delimiter.square.Podlang"], // For arrays/sets
      [/[\\{\\}]/, "delimiter.curly.Podlang"], // For dictionaries
      [/[()]/, "delimiter.parenthesis.Podlang"],
      [/,/, "delimiter.comma.Podlang"],
      [/:/, "delimiter.colon.Podlang"],

      // Other symbols treated as operators if not covered above
      [/[=><!~?&|+*/%#^\\-]+/, "operator.Podlang"]
    ],

    comment: [
      [/[^\\/*]+/, "comment.block.Podlang"],
      [/\/\*/, "comment.block.Podlang", "@push"],
      ["\\\\*/", "comment.block.Podlang", "@pop"],
      [/[/*]/, "comment.block.Podlang"]
    ],

    string_double: [
      [/[^\\\\"]+/, "string.quoted.double.Podlang"],
      [/@escapes/, "string.escape.char.Podlang"],
      [/\\\\./, "string.escape.invalid.Podlang"],
      [
        /"/,
        {
          token: "string.quoted.double.Podlang",
          bracket: "@close",
          next: "@pop"
        }
      ]
    ],

    string_single: [
      [/[^\\\\']+/, "string.quoted.single.Podlang"],
      [/@escapes/, "string.escape.char.Podlang"],
      [/\\\\./, "string.escape.invalid.Podlang"],
      [
        /'/,
        {
          token: "string.quoted.single.Podlang",
          bracket: "@close",
          next: "@pop"
        }
      ]
    ],

    whitespace: [
      [/[ \\t\\r\\n]+/, "white"],
      [/\/\//, "comment.line.Podlang", "@commentLine"]
    ],

    commentLine: [[/.*/, "comment.line.Podlang", "@pop"]]
  }
};

/**
 * Convert API Diagnostic to Monaco MarkerData
 */
export function convertDiagnosticToMarker(
  diagnostic: Diagnostic
): monaco.editor.IMarkerData {
  let severity: monaco.MarkerSeverity;

  switch (diagnostic.severity) {
    case "Error":
      severity = monaco.MarkerSeverity.Error;
      break;
    case "Warning":
      severity = monaco.MarkerSeverity.Warning;
      break;
    case "Information":
      severity = monaco.MarkerSeverity.Info;
      break;
    case "Hint":
      severity = monaco.MarkerSeverity.Hint;
      break;
    default:
      severity = monaco.MarkerSeverity.Info;
  }

  return {
    message: diagnostic.message,
    severity,
    startLineNumber: diagnostic.start_line,
    startColumn: diagnostic.start_column,
    endLineNumber: diagnostic.end_line,
    endColumn: diagnostic.end_column
  };
}

/**
 * Convert array of diagnostics to Monaco markers
 */
export function convertDiagnosticsToMarkers(
  diagnostics: Diagnostic[]
): monaco.editor.IMarkerData[] {
  return diagnostics.map(convertDiagnosticToMarker);
}

/**
 * Setup Monaco editor with Podlang language support
 */
export function setupMonacoEditor(
  _editor: monaco.editor.IStandaloneCodeEditor,
  monacoInstance: typeof import("monaco-editor")
): void {
  // Register the Podlang language
  monacoInstance.languages.register({ id: "Podlang" });

  // Set up syntax highlighting
  monacoInstance.languages.setMonarchTokensProvider(
    "Podlang",
    PodlangMonarchLanguage
  );

  console.log("Podlang language registered and Monarch tokens set");
}

/**
 * Update Monaco editor markers for diagnostics
 */
export function updateEditorMarkers(
  editor: monaco.editor.IStandaloneCodeEditor,
  monacoInstance: typeof import("monaco-editor"),
  diagnostics: Diagnostic[]
): void {
  const model = editor.getModel();
  if (!model) {
    console.warn("No model available for updating markers");
    return;
  }

  const markers = convertDiagnosticsToMarkers(diagnostics);
  monacoInstance.editor.setModelMarkers(model, "Podlang-validator", markers);
}

/**
 * Clear all markers from the editor
 */
export function clearEditorMarkers(
  editor: monaco.editor.IStandaloneCodeEditor,
  monacoInstance: typeof import("monaco-editor")
): void {
  const model = editor.getModel();
  if (!model) return;

  monacoInstance.editor.setModelMarkers(model, "Podlang-validator", []);
}

/**
 * Default content for new files
 */
export function getDefaultEditorContent(): string {
  return `// Welcome to the POD Editor!
// Write your Podlang queries here to create and test PODs.
//
// Example:
// REQUEST(
//     Equal(?pod1["field1"], ?pod2["field1"])
//     Lt(?pod1["timestamp"], 1234567890)
// )

`;
}
