# Public source and release checklist

The public repository starts from a reviewed current product snapshot. Earlier
private Git history, local agent state and credentials are not imported.
Version numbering continues from development preview 0.1.53; the next normal
controller-assigned patch is 0.1.54, not a reset of product development.

- Scan every outgoing tracked file and new commit for private references and secrets.
- Exclude agent instructions, credentials, local configuration, recovery data,
  customer PDFs and real signature assets from source and release packages.
- Keep tests and examples synthetic or explicitly redistributable.
- Preserve Apache-2.0 ownership notices, the combined Slint GPL-3.0-only route,
  PDFium source provenance and all third-party license notices.
- Use the established release controller and one release authority.
- Verify tag, version, source revision, checksums and actual release downloads.
- Public hosted validation is independent of native desktop/printer acceptance.
- Keep Fedora source builds and native Windows qualification as separate stages.
- Never promote development previews to stable without their acceptance gates.
