# Windows installer build: SUCCESS

Source: `ci/windows-installer` / `847d1c581bf80ce0e66f2d25d9242a4b4b7e3333`

Run: https://github.com/w3fans/dPDF/actions/runs/43551199504

The build job installs the per-user MSI on a isolated runner profile, runs the
installed dpdf.exe (version, PDFium probe, bundled qpdf extract), checks the
Open With registration and Start Menu shortcut, uninstalls and checks cleanup.
A FAILURE or CANCELLED result means no installer is qualified for this commit.

```
e7bea5b73ed937a2835d1e9cf9ba4e15bd5cbe674f8e688893251fbde0eb16ff  build-provenance.json
59dffd7844c229a91782bed0690b5979c6d3fc0f26467cc2a4a96d61e79b66df  dPDF-0.1.54-windows-x64-portable.zip
bd1810adfcce8e95a483ced2765c50851394d4173012b8ec6e9c936a1f12e5f8  dPDF-0.1.54-windows-x64.msi
```
