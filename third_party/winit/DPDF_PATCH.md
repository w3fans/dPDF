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
Registry cache files and agent state are not part of this copy. Cargo.toml is
the published normalized manifest; its original is retained as Cargo.toml.orig.
This is a downstream patch, not an upstream Winit release.
