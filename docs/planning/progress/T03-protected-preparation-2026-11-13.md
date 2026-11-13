# T03 early protected preparation increment

Status: IN_PROGRESS; current compiled/runtime validation and experienced lifecycle
review PENDING. No release/version/tag or production baking.

Native XTest Ctrl+P during initial protected review decryption reproduced blank
page 25 until two controlled helpers were released on source 2d75b74b (Linux
merge b067f63f, executable 1208b78a). Both helpers reaped, encrypted source
unchanged. Viewport OCR blank pending, page 25 visible after release. See
T03-protected-preparation-stall.json.

All six print/export commands now queue missing protected bytes through the same
two-lease helper pool and resume only after matching document/request completion.
The reader accessor no longer invokes synchronous decryption. Pending commands
own no native handles; admission/failure sends a terminal print/export result.
Close cancels retained commands and clears UI busy flags. Shutdown cancels queued
preparation before it can launch a child. Existing final detached print/export
threads and overlay/image helper preparation remain separate unfinished work.

Added generated protected-print shutdown and protected-export close cases to the
existing four helper modes. They require page 25 while initial decryption stays
stalled, all children reaped, no cancelled queued child started, source hash and
pre-existing destination unchanged. GTK is file-backend-only; no print submitted.

Seven blobs matched public candidate 6f54acf8 readback. Linux run 43512132506
caught a missing exhaustive recovery match for the new result variant. Corrected
that handler in f13080e049bf8261bc489cfb17dbe445006d9137, tree
d43290b1ffcebf974fe01cb33b39b4ec2ee019bc; worker blob 9023baabcadc281f49e0a7335c5837c991d200cf
matches local readback. Original failed run retained; new validation is required.

Local cargo fmt --check, diff --check, Python syntax and release metadata verify
passed. No local Rust compilation performed. PR #10 stays draft; no experienced
review submission or owner GO received. Review must cover current command
retention/resumption, shared admission, stale-result routing and teardown order.
