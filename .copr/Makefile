# COPR source-preparation sandbox only; not a workstation build/install command.
srpm:
	dnf -y install cargo rust rpm-build python3 git-core gcc
	python3 scripts/build-srpm.py --outdir "$(outdir)"
