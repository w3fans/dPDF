# Windows installer build: FAILURE

Source: `v0.1.54` / `e06b05b5f1e91e9e9e99b97967842bad4996bc67`

Run: https://github.com/w3fans/dPDF/actions/runs/42486810035

The build job installs the per-user MSI on a isolated runner profile, runs the
installed dpdf.exe (version, PDFium probe, bundled qpdf extract), checks the
Open With registration and Start Menu shortcut, uninstalls and checks cleanup.
A FAILURE or CANCELLED result means no installer is qualified for this commit.

