# Windows installer build: SUCCESS

Source: `v0.1.54` / `e06b05b5f1e91e9e9e99b97967842bad4996bc67`

Run: https://github.com/w3fans/dPDF/actions/runs/42487270092

The build job installs the per-user MSI on a isolated runner profile, runs the
installed dpdf.exe (version, PDFium probe, bundled qpdf extract), checks the
Open With registration and Start Menu shortcut, uninstalls and checks cleanup.
A FAILURE or CANCELLED result means no installer is qualified for this commit.

```
383c748787d660f24a04c8404eb407f16fcc3abdc560ef01617da347391ee0b3  build-provenance.json
23b65559f0f35870e3f929e37d5da2817ddffd7b1e8b08c693554fbbf5dbc81f  dPDF-0.1.54-windows-x64-portable.zip
9f6366be7763367cf43b7868262df6eff7b5b38a55a8b20fd84e477dcb1cfdd5  dPDF-0.1.54-windows-x64.msi
```
