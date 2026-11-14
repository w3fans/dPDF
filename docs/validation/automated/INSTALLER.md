# Windows installer build: SUCCESS

Source: `ci/windows-installer` / `6e5b15e453faceb40b161ed4ec18af932029b3f3`

Run: https://github.com/w3fans/dPDF/actions/runs/43570491723

The build job installs the per-user MSI on a isolated runner profile, runs the
installed dpdf.exe (version, PDFium probe, bundled qpdf extract), checks the
Open With registration and Start Menu shortcut, uninstalls and checks cleanup.
A FAILURE or CANCELLED result means no installer is qualified for this commit.

```
03dacfed2666ed15c547595b255e81b6cb55510c35f72b45ef44961c3510b636  build-provenance.json
a4f769d9d83a421c69aa6ed343e59d47068103631ad45bfc110bec36d7376dfd  dPDF-0.1.54-windows-x64-portable.zip
4153558d2304e9d7f51457de9ad9991f9a91afef58842af34002d6d19b6f4b6e  dPDF-0.1.54-windows-x64.msi
```
