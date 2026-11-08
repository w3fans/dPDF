"""Checksum large release inputs without buffering the whole file."""
import hashlib


def sha256_file(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()
