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


## Runtime fixture correction and current source

Run 43512520241 on f13080e0 passed compilation/Clippy, all 88 debug/release
unit tests and native Windows hosted checks. Linux's first four helper cases
passed (close/retry/shutdown/recovery). The new protected-print case failed before
queueing: Application smoke mode deliberately disables native print admission.
This was a diagnostic setup error, not proof of protected-reader responsiveness.
No new protected case is marked passed.

Enable admission only around the controlled native request, restoring the smoke
setting immediately; GTK stays restricted to the file backend and the held
preparation must be cancelled before any dialog starts. Integrated qualified
parser main 1adea6f as a second parent in current head
8a26ab45ff5fba5cfcaef8621a55706daac5033e, tree
7f4cc3858904e0ea4bfe288c7085e8a68f1e1ad3. Current worker, fixture, lock,
inventories, qualification files and focused changelog matched remote readback.
Full current-source validation and native corrected replay remain PENDING.

Current protected candidate validation: run 43514021130 on head 8a26ab45; live at last authoritative poll.


## Qualified protected increment

Head 8a26ab45 passed every job in run 43514021130: Linux 132191210971,
Windows 132191211206 and policy 132191211248. All six helper cases passed:
close 0.34s, retry 0.22s, shutdown 0.17s, recovery 0.30s, protected print 0.20s,
protected export 0.22s. Both protected cases rendered page 25 before cancellation,
reaped the sole started child, never launched cancelled queued decryption and
preserved source/existing destination bytes.

Verified Linux archive 14079194303, SHA256
ad828ae1c30e006ece8ec83034b30060e3e42f06f24ffcb2978ebde03ebdfe2a;
tested merge 32f59f95aeb8cda6c661b85ea7cea41b29ce6b6e, executable SHA256
6d68ced5b923e2f56949e8afa4ece3442640d34ce6142454b6cdfaff65f58e8b.
Replayed original native Ctrl+P/page-25 input on that binary. Page body OCR shows
page 25 before any helper release, and native Ctrl+W cancels/reaps its pending
preparation. Exactly one decrypt started; no print dialog/submission occurred.
Source unchanged. See T03-protected-responsive-replay.json. Required lifecycle
review is still PENDING; this does not qualify the following overlay increment.
