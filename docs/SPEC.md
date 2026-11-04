# The `.van` Plugin Format

**Version:** 1 (Draft)

## 1. Scope

This document defines the `.van` plugin archive format. It covers the file name, the archive structure, and the manifest. It does not define the package manager, the plugin runtime, dependency installation, or trust/signing. Those are separate concerns and are explicitly out of scope for version 1.

## 2. File Name

A `.van` archive is named:

```
<name>-<version>.van
```

Both `<name>` and `<version>` are read from `manifest.json`. The build tool is responsible for constructing the file name; developers do not choose it manually.

## 3. Container

A `.van` file is a ZIP archive.

- Compression: `store` or `deflate` is permitted.
- Encryption: MUST NOT be used.
- A `.van` archive MUST NOT contain a `node_modules` directory at any depth.

## 4. Archive Structure

Every `.van` archive has exactly this layout:

```
<name>-<version>.van/
├── manifest.json
└── dist/
```

Rules:

- `manifest.json` MUST exist at the archive root.
- `dist/` MUST exist at the archive root.
- **Everything inside `dist/` is the plugin.** Files outside `dist/` (other than `manifest.json`) are not part of the plugin.
- The build tool creates `dist/` and copies the plugin files into it. Developers do not assemble the archive layout by hand.

Example:

```
examplePlugin-0.1.0.van/
├── manifest.json
└── dist/
    ├── package.json
    └── src/
        └── app.ts
```

## 5. Manifest

`manifest.json` is a single JSON object.

- Encoding: UTF-8, no byte order mark.
- MUST conform to RFC 8259.
- MUST NOT contain comments, duplicate member names, or trailing data after the object.

### 5.1 Required Members

| Member       | Type   | Description                                             |
| ------------ | ------ | ------------------------------------------------------- |
| `name`       | string | npm package name.                                       |
| `version`    | string | Semantic Versioning 2.0.0 version string.               |
| `entrypoint` | string | Path to the plugin entry file, **relative to `dist/`**. |

Unrecognized members MUST be ignored by readers.

### 5.2 Example

```json
{
  "name": "example-plugin",
  "version": "0.1.0",
  "entrypoint": "src/app.ts"
}
```

In this example, the entry file inside the archive is `dist/src/app.ts`.

## 6. Path Rules

Archive entry names MUST:

- be valid UTF-8 in Unicode NFC,
- use `/` as the path separator (no backslashes),
- be relative (no leading `/`, no drive letters),
- not contain empty, `.`, or `..` segments,
- be unique within the archive,
- not represent symbolic links or other non-regular files.

Readers MUST reject archives that violate these rules. These rules prevent path traversal ("zip slip") and ensure the archive can be extracted safely.

## 7. Out of Scope for Version 1

The following are intentionally not part of version 1:

- Integrity verification (checksums, hash inventories)
- Signatures and trust model
- npm dependency installation details
- Plugin runtime API

They may be added in a future version.

## 8. Implementation Checklist for Coding Agents

When building or consuming a `.van` archive, an implementation MUST:

1. Read `manifest.json` from the archive root.
2. Validate that `name`, `version`, and `entrypoint` are present and correctly typed.
3. Build or verify the file name as `<name>-<version>.van`.
4. Reject archives without a `dist/` directory.
5. Reject archives containing `node_modules` at any depth.
6. Reject entry names that violate the path rules in Section 6.
7. Ignore any manifest members not listed in Section 5.1.
