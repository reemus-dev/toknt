import eslint from "@eslint/js";
import tseslint from "typescript-eslint";
import configPrettier from "eslint-config-prettier";
import pluginImportX from "eslint-plugin-import-x";
import pluginN from "eslint-plugin-n";
import pluginUnicorn from "eslint-plugin-unicorn";
import pluginUnusedImports from "eslint-plugin-unused-imports";
import pluginSimpleImportSort from "eslint-plugin-simple-import-sort";
import globals from "globals";

const NODE_VERSION = ">=24";

const FILE_IGNORES = [
  "**/*.cjs",
  "**/*.js",
  "**/*.mjs",
  "**/.archive/",
  "eslint.config.js",
  ".claude/",
  ".archive/",
  ".task/",
  ".agents/",
  "docs/",
  "dist/",
];

const ALLOW_DEFAULT_PROJECT = [
  // "shared/*"
];

const IMPORT_RULES = {
  //#── Sorting
  "sort-imports": "off",
  "simple-import-sort/imports": [
    "error",
    {
      groups: [
        [
          "^\\u0000", // Side effect imports
          "^node:", // Node.js built-ins
          "^\\w", // External packages: unscoped
          "^@(?!/)", // External packages: scoped
          "^@/", // Internal path alias (@/)
          "^\\.", // Relative imports
        ],
      ],
    },
  ],
  //#── Unused
  "unused-imports/no-unused-imports": "off",
  "unused-imports/no-unused-vars": [
    "off",
    {
      vars: "all",
      varsIgnorePattern: "^_",
      args: "after-used",
      argsIgnorePattern: "^_",
    },
  ],
  //#── Restrictions
  "import-x/order": "off",
  "import-x/no-unresolved": [
    "error",
    {},
  ],
  "import-x/extensions": [
    "warn",
    "ignorePackages",
    {ts: "never", tsx: "never"},
  ],
  "import-x/no-restricted-paths": [
    "error",
    {},
  ],
};

const NODE_COMPAT_RESTRICT_BUN = [
  {
    name: "bun",
    message:
      "Do not import Bun runtime APIs. This repo must stay Node.js-compatible.",
  },
  {
    name: "bun:*",
    message:
      "Do not import Bun runtime APIs. This repo must stay Node.js-compatible.",
  },
];
const NODE_COMPAT_RULES = {
  "no-restricted-globals": [
    "error",
    {
      name: "Bun",
      message:
        "Do not use Bun runtime APIs. This repo must stay Node.js-compatible.",
    },
  ],
  "n/no-restricted-import": ["error", NODE_COMPAT_RESTRICT_BUN],
  "n/no-restricted-require": ["error", NODE_COMPAT_RESTRICT_BUN],
  "n/no-unsupported-features/node-builtins": ["error", {version: NODE_VERSION}],
};

export default tseslint.config(
  {ignores: FILE_IGNORES},

  eslint.configs.recommended,
  tseslint.configs.recommendedTypeChecked,
  pluginImportX.flatConfigs.recommended,
  pluginImportX.flatConfigs.typescript,

  {
    languageOptions: {
      ecmaVersion: 2023,
      sourceType: "module",
      globals: {
        ...globals.browser,
        ...globals.es2023,
      },
      parserOptions: {
        sourceType: "module",
        projectService: {
          // allowDefaultProject: ALLOW_DEFAULT_PROJECT,
        },
        // tsconfigRootDir: import.meta.dirname,
      },
    },
  },

  {
    files: ["**/*.ts", "**/*.tsx"],
    plugins: {
      "@typescript-eslint": tseslint.plugin,
      n: pluginN,
      unicorn: pluginUnicorn,
      "unused-imports": pluginUnusedImports,
      "simple-import-sort": pluginSimpleImportSort,
    },
    rules: {
      // ── Javascript ───────────────────────────────────────────────────────
      "constructor-super": "warn",
      "for-direction": "warn",
      "getter-return": "warn",
      "use-isnan": "warn",
      "valid-typeof": "warn",
      "no-unused-vars": "off",
      "no-async-promise-executor": "warn",
      "no-class-assign": "warn",
      "no-compare-neg-zero": "warn",
      "no-constant-condition": "warn",
      "no-control-regex": "warn",
      "no-debugger": "warn",
      "no-dupe-args": "warn",
      "no-dupe-class-members": "warn",
      "no-dupe-else-if": "warn",
      "no-dupe-keys": "warn",
      "no-duplicate-case": "warn",
      "no-empty-character-class": "warn",
      "no-empty-pattern": "warn",
      "no-ex-assign": "warn",
      "no-fallthrough": "warn",
      "no-func-assign": "warn",
      "no-import-assign": "warn",
      "no-redeclare": "off",
      "no-invalid-regexp": "warn",
      "no-irregular-whitespace": "warn",
      "no-loss-of-precision": "warn",
      "no-misleading-character-class": "warn",
      "no-nested-ternary": "off",
      "no-obj-calls": "warn",
      "no-prototype-builtins": "warn",
      "no-self-assign": "warn",
      "no-sparse-arrays": "warn",
      "no-undef": "off",
      "no-unexpected-multiline": "warn",
      "no-unreachable": "warn",
      "no-unsafe-finally": "warn",
      "no-unsafe-negation": "warn",
      "no-unsafe-optional-chaining": "warn",
      "no-useless-backreference": "warn",

      // ── Typescript ───────────────────────────────────────────────────────
      "@typescript-eslint/ban-ts-comment": "off",
      "@typescript-eslint/no-explicit-any": "off",
      "@typescript-eslint/no-namespace": "off",
      "@typescript-eslint/no-redundant-type-constituents": "off",
      "@typescript-eslint/no-unsafe-argument": "off",
      "@typescript-eslint/no-unsafe-assignment": "off",
      "@typescript-eslint/no-unsafe-call": "off",
      "@typescript-eslint/no-unsafe-member-access": "off",
      "@typescript-eslint/no-unsafe-return": "off",
      "@typescript-eslint/no-unused-vars": "off",
      "@typescript-eslint/require-await": "off",
      "@typescript-eslint/restrict-template-expressions": "off",
      "@typescript-eslint/adjacent-overload-signatures": "warn",
      "@typescript-eslint/await-thenable": "warn",
      "@typescript-eslint/no-extra-non-null-assertion": "warn",
      "@typescript-eslint/no-inferrable-types": "warn",
      "@typescript-eslint/no-misused-new": "warn",
      "@typescript-eslint/no-misused-promises": "warn",
      "@typescript-eslint/no-non-null-asserted-nullish-coalescing": "warn",
      "@typescript-eslint/no-non-null-asserted-optional-chain": "warn",
      "@typescript-eslint/no-unnecessary-type-assertion": "warn",
      "@typescript-eslint/prefer-as-const": "warn",
      "@typescript-eslint/prefer-for-of": "warn",
      "@typescript-eslint/prefer-includes": "warn",
      "@typescript-eslint/prefer-optional-chain": "warn",
      "@typescript-eslint/prefer-string-starts-ends-with": "warn",
      "@typescript-eslint/restrict-plus-operands": "warn",
      "@typescript-eslint/naming-convention": [
        "error",
        {
          selector: "default",
          format: ["camelCase", "UPPER_CASE", "snake_case", "PascalCase"],
          leadingUnderscore: "allow",
          trailingUnderscore: "allow",
        },
        {
          selector: "variable",
          format: ["camelCase", "UPPER_CASE", "snake_case", "PascalCase"],
          leadingUnderscore: "allow",
          trailingUnderscore: "allow",
        },
        {
          selector: "typeLike",
          format: ["PascalCase"],
        },
        {
          selector: "property",
          leadingUnderscore: "allow",
          format: null,
        },
      ],

      // ── Unicorn ──────────────────────────────────────────────────────────
      ...pluginUnicorn.configs.recommended.rules,
      "unicorn/catch-error-name": "off",
      "unicorn/consistent-destructuring": "off",
      "unicorn/consistent-function-scoping": "off",
      "unicorn/custom-error-definition": "off",
      "unicorn/filename-case": "off",
      "unicorn/import-index": "off",
      "unicorn/no-array-callback-reference": "off",
      "unicorn/no-array-for-each": "off",
      "unicorn/no-array-push-push": "off",
      "unicorn/no-await-expression-member": "off",
      "unicorn/no-keyword-prefix": "off",
      "unicorn/no-nested-ternary": "off",
      "unicorn/no-null": "off",
      "unicorn/no-process-exit": "off",
      "unicorn/no-unsafe-regex": "off",
      "unicorn/no-unused-properties": "off",
      "unicorn/prefer-at": "off",
      "unicorn/prefer-export-from": "off",
      "unicorn/prefer-global-this": "off",
      "unicorn/prefer-spread": "off",
      "unicorn/prefer-string-replace-all": "off",
      "unicorn/prefer-top-level-await": "off",
      "unicorn/prefer-type-error": "off",
      "unicorn/prevent-abbreviations": "off",
      "unicorn/require-post-message-target-origin": "off",
      "unicorn/string-content": "off",
      "unicorn/template-indent": "warn",
      "unicorn/import-style": "error",
      "unicorn/no-abusive-eslint-disable": "off",
      "unicorn/no-array-method-this-argument": "error",
      ...IMPORT_RULES,
      ...NODE_COMPAT_RULES,
    },
  },

  // Prettier must be the last. It disables conflicting stylistic rules.
  configPrettier,
);
