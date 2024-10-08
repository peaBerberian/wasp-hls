module.exports = {
  env: {
    browser: true,
    es6: true,
  },
  extends: [
    "plugin:@typescript-eslint/recommended",
    "plugin:@typescript-eslint/recommended-requiring-type-checking",
    "prettier",
  ],
  parser: "@typescript-eslint/parser",
  parserOptions: {
    project: "tsconfig.json",
    sourceType: "module",
  },
  plugins: [
    "eslint-plugin-import",
    "eslint-plugin-jsdoc",
    "@typescript-eslint",
  ],
  rules: {
    "@typescript-eslint/adjacent-overload-signatures": "error",
    "@typescript-eslint/array-type": [
      "error",
      {
        default: "array-simple",
      },
    ],
    "@typescript-eslint/await-thenable": "error",
    "@typescript-eslint/consistent-type-assertions": [
      "error",
      {
        assertionStyle: "as",
        objectLiteralTypeAssertions: "allow",
      },
    ],
    "@typescript-eslint/prefer-promise-reject-errors": "error",
    "@typescript-eslint/only-throw-error": "error",
    "@typescript-eslint/consistent-type-definitions": "error",
    "@typescript-eslint/dot-notation": "error",
    "@typescript-eslint/explicit-member-accessibility": [
      "off",
      {
        accessibility: "explicit",
      },
    ],
    "@typescript-eslint/consistent-type-imports": "error",
    "@typescript-eslint/consistent-type-exports": "error",
    "@typescript-eslint/naming-convention": [
      "error",
      {
        selector: "property",
        format: ["camelCase", "UPPER_CASE", "PascalCase"],
        leadingUnderscore: "allowSingleOrDouble",
        trailingUnderscore: "allowSingleOrDouble",
      },
      {
        selector: "method",
        format: ["camelCase"],
        leadingUnderscore: "allowSingleOrDouble",
        trailingUnderscore: "allowSingleOrDouble",
      },
      {
        selector: "classMethod",
        format: ["camelCase"],
        leadingUnderscore: "allowSingleOrDouble",
        trailingUnderscore: "allowSingleOrDouble",
      },
      {
        selector: "variable",
        format: ["camelCase", "UPPER_CASE", "PascalCase"],
        leadingUnderscore: "allowSingleOrDouble",
        trailingUnderscore: "allowSingleOrDouble",
      },
      {
        selector: "parameter",
        format: ["camelCase"],
        leadingUnderscore: "allowSingleOrDouble",
      },

      {
        selector: "memberLike",
        modifiers: ["private"],
        format: ["camelCase"],
        leadingUnderscore: "require",
      },
      {
        selector: "enum",
        format: ["PascalCase", "UPPER_CASE"],
        leadingUnderscore: "allowSingleOrDouble",
      },
      {
        selector: "typeLike",
        format: ["PascalCase"],
        leadingUnderscore: "allowSingleOrDouble",
      },
    ],
    "@typescript-eslint/no-empty-function": "error",
    "@typescript-eslint/no-empty-interface": "error",
    "@typescript-eslint/no-explicit-any": "error",
    "@typescript-eslint/no-extraneous-class": "error",
    "@typescript-eslint/no-floating-promises": "error",
    "@typescript-eslint/no-for-in-array": "error",
    "@typescript-eslint/no-inferrable-types": "off",
    "@typescript-eslint/no-misused-new": "error",
    "@typescript-eslint/no-namespace": "error",
    "@typescript-eslint/no-non-null-assertion": "error",
    "@typescript-eslint/no-this-alias": "error",
    // Might be enabled in the future, for now this is too much work:
    "@typescript-eslint/no-unsafe-enum-comparison": ["off"],
    "@typescript-eslint/no-unnecessary-boolean-literal-compare": "error",
    "@typescript-eslint/no-unnecessary-qualifier": "error",
    "@typescript-eslint/no-unnecessary-type-arguments": "error",
    "@typescript-eslint/no-unnecessary-type-assertion": "error",
    "@typescript-eslint/no-unused-expressions": "error",
    "@typescript-eslint/no-use-before-define": "off",
    "@typescript-eslint/no-var-requires": "error",
    "@typescript-eslint/prefer-for-of": "off",
    "@typescript-eslint/prefer-function-type": "error",
    "@typescript-eslint/prefer-namespace-keyword": "error",
    "@typescript-eslint/prefer-readonly": "off",
    "@typescript-eslint/promise-function-async": "off",
    "@typescript-eslint/no-unused-vars": [
      "error",
      {
        args: "all",
        argsIgnorePattern: "^_",
        caughtErrors: "none",
        destructuredArrayIgnorePattern: "^_",
        varsIgnorePattern: "^_",
      },
    ],
    "@typescript-eslint/no-shadow": ["error"],
    "@typescript-eslint/restrict-plus-operands": "error",
    "@typescript-eslint/strict-boolean-expressions": "error",
    "@typescript-eslint/triple-slash-reference": [
      "error",
      {
        path: "always",
        types: "prefer-import",
        lib: "always",
      },
    ],
    "@typescript-eslint/unbound-method": "error",
    "@typescript-eslint/unified-signatures": "error",
    "arrow-body-style": "off",
    "arrow-parens": ["off", "always"],
    complexity: [
      "off",
      {
        max: 20,
      },
    ],
    "constructor-super": "error",
    curly: "error",
    "default-case": "off",
    eqeqeq: "error",
    "guard-for-in": "warn",
    "id-blacklist": "off",
    "id-match": "off",
    "import/no-default-export": "off",
    "import/no-deprecated": "error",
    "import/no-extraneous-dependencies": [
      "error",
      {
        devDependencies: ["**/*.test.ts", "**/__tests__/**", "demo/**/*"],
      },
    ],
    "import/no-internal-modules": "off",
    "import/order": [
      "error",
      {
        alphabetize: {
          order: "asc",
          caseInsensitive: true,
        },
      },
    ],
    "jsdoc/check-alignment": "error",
    "jsdoc/no-types": "off",
    "max-classes-per-file": ["warn", 5],
    "max-lines": ["off", 300],
    "newline-per-chained-call": "off",
    "no-bitwise": "off",
    "no-caller": "error",
    "no-console": "error",
    "no-debugger": "error",
    "no-duplicate-case": "error",
    "no-empty": "error",
    "no-eval": "error",
    "no-fallthrough": "error",
    "no-invalid-this": "error",
    "no-magic-numbers": "off",
    "no-new-wrappers": "error",
    "no-nested-ternary": "error",
    "no-param-reassign": "error",
    "no-return-await": "error",
    "no-sequences": "error",
    // eslint has issues with enums, @typescript-eslint/no-shadow works better
    "no-shadow": "off",
    "no-sparse-arrays": "error",
    "no-template-curly-in-string": "error",
    "no-throw-literal": "error",
    "no-undef-init": "error",
    "no-unsafe-finally": "error",
    "no-unused-labels": "error",
    "no-var": "error",
    "no-void": "error",
    "object-shorthand": "error",
    "one-var": ["error", "never"],
    "prefer-const": "error",
    "prefer-spread": "error",
    "prefer-object-spread": "error",
    "prefer-template": "off",
    radix: "error",
    "spaced-comment": [
      "error",
      "always",
      {
        markers: ["/"],
      },
    ],
    "use-isnan": "error",
    "valid-typeof": "error",
    yoda: "error",
  },
};
