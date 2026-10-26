"""Regenerate the public synthetic ADCRYPT fixture using OpenSSL, not Exhume.

Only regeneration requires OpenSSL. Tests use the checked-in bytes.
"""

import hashlib
import hmac
from pathlib import Path
import struct
import subprocess
import zlib


ROOT = Path(__file__).resolve().parent
PASSWORD = b" fixture password "


def checksum(data):
    return data + struct.pack("<I", zlib.adler32(data))


def section(kind, offset, payload=b"", terminal=False):
    size = 76 + len(payload)
    descriptor = kind.encode().ljust(16, b"\0")
    descriptor += struct.pack("<QQ", offset if terminal else offset + size, 0 if terminal else size)
    return checksum(descriptor.ljust(72, b"\0")) + payload


def segment(index, data):
    result = b"EVF\x09\x0d\x0a\xff\0" + struct.pack("<BHH", 1, index + 1, 0)
    if index == 0:
        volume = checksum(struct.pack("<IIIIQ", 0, 2, 1, 512, 2))
        result += section("volume", len(result), volume)
    chunk_offset = len(result) + 76
    result += section("sectors", len(result), checksum(data))
    table = checksum(struct.pack("<IIQI", 1, 0, 0, 0))
    table += checksum(struct.pack("<I", chunk_offset))
    result += section("table", len(result), table)
    return result + section("next" if index == 0 else "done", len(result), terminal=True)


def encrypt_ctr(key, data, segment_number=0):
    # ADCRYPT counts AES blocks little-endian; OpenSSL's built-in CTR uses BE.
    counters = b"".join(
        ((segment_number << 64) + block).to_bytes(16, "little")
        for block in range((len(data) + 15) // 16)
    )
    stream = subprocess.run(
        ["openssl", "enc", "-aes-256-ecb", "-K", key.hex(), "-nosalt", "-nopad"],
        input=counters, capture_output=True, check=True,
    ).stdout
    return bytes(a ^ b for a, b in zip(data, stream))


def main():
    disk = bytearray(1024)
    disk[446:462] = struct.pack("<B3sB3sII", 0, b"\0" * 3, 0x83, b"\0" * 3, 1, 1)
    disk[510:512] = b"\x55\xaa"
    marker = b"Encrypted EWF release fixture.\n"
    disk[512:512 + len(marker)] = marker
    (ROOT / "disk.raw").write_bytes(disk)

    salt = bytes(range(16))
    file_key = bytes(range(32))
    iterations = 1000
    wrapping_key = hashlib.pbkdf2_hmac("sha1", hashlib.sha256(PASSWORD).digest(), salt, iterations, 32)
    encrypted_key = encrypt_ctr(wrapping_key, file_key)
    key_hmac = hmac.digest(wrapping_key, encrypted_key, "sha256")
    header = bytearray(48)
    header[:8] = b"ADCRYPT\0"
    struct.pack_into("<II", header, 8, 1, 512)
    struct.pack_into("<IIIIII", header, 24, 3, 1, iterations, 16, 32, 32)
    header = bytes(header + salt + encrypted_key + key_hmac).ljust(512, b"\0")
    for index in range(2):
        plain = segment(index, disk[index * 512:(index + 1) * 512])
        encrypted = encrypt_ctr(file_key, plain, index)
        (ROOT / f"encrypted.E0{index + 1}").write_bytes((header if index == 0 else b"") + encrypted)


if __name__ == "__main__":
    main()
