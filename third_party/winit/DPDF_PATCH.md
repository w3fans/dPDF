# Winit 0.30.13 Wayland receive patch

Base: upstream e9809ef54b18499bb4f2cac945719ecc2a61061b.
Crates.io archive SHA256: a6755fa58a9f8350bd1e472d4c3fcc25f824ec358933bba33306d0b63df5978d.
The published upstream Apache-2.0 license text is retained unchanged.

Slint 1.18 uses Winit 0.30, whose Wayland backend does not bind the
wl_data_device_manager receive protocol. This local patch adds incoming local
text/uri-list drops only, using the existing SCTK dependency and event loop.
It does not read clipboard selections or implement outgoing drag operations.
Transfer size, lifetime and concurrent transfers are bounded; files are opened
only after a completed drop. X11 and Windows code stays upstream.

The original crate archive is independently checksum-verified before extraction.
Registry cache files and agent state are not part of this copy. Cargo.toml derives from the published normalized manifest with two downstream
feature additions: activate the existing optional percent-encoding dependency for
Wayland, and enable fs on the existing Unix rustix dependency for nonblocking
receive setup. Dependency versions are unchanged. Cargo.toml.orig retains the
unchanged upstream original manifest.
This is a downstream patch, not an upstream Winit release.


A fresh archive comparison on 14 November 2026 found only those two normalized
manifest changes, three modified Wayland integration modules, and the added
Wayland file_drop.rs and uri_list.rs implementation files. DPDF_PATCH.md is the
local provenance note; upstream .cargo_vcs_info.json is omitted from this copy,
and its revision is recorded above. All other upstream files, including LICENSE
and X11/Windows source, match byte-for-byte. This verifies patch provenance, not
native Wayland behavior or a security qualification.
