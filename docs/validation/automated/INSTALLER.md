# Windows installer build: SUCCESS

Source: `ci/windows-installer` / `6e3e9cc31ca9575978c7401149aaccef29050ea3`

Run: https://github.com/w3fans/dPDF/actions/runs/43554581249

The build job installs the per-user MSI on a isolated runner profile, runs the
installed dpdf.exe (version, PDFium probe, bundled qpdf extract), checks the
Open With registration and Start Menu shortcut, uninstalls and checks cleanup.
A FAILURE or CANCELLED result means no installer is qualified for this commit.

```
27d8f0824f67053186a837b9184a6bfe1133fd9816fd7f4aa2ff29d2f39d2396  build-provenance.json
f6c8cde4ea95c37f16b82e6bfaf6a6f1e521e78d741cd5ef4519287f65c343ce  dPDF-0.1.54-windows-x64-portable.zip
95cce8060c27612436ecc2f5c3fb27700d221d5675c419b6c960adcbbd03d2c4  dPDF-0.1.54-windows-x64.msi
```
